use super::chains::inference_chain_trait::LLMInferenceResponse;
use super::prompts::prompts::{JobPromptGenerator, Prompt};
use crate::agent::error::AgentError;
use crate::agent::job::Job;
use crate::agent::parsing_helper::ParsingHelper;
use crate::agent::{agent::Agent, job_manager::JobManager};
use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::vector_fs::vector_fs::VectorFS;
use crate::vector_fs::vector_fs_error::VectorFSError;
use serde_json::{Map, Value as JsonValue};
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::collections::HashMap;
use std::result::Result::Ok;
use std::sync::Arc;
use tracing::instrument;

impl JobManager {
    /// Attempts to extract multiple keys from the inference response, including retry inferencing/upper + lower if necessary.
    /// Potential keys hashmap should have the expected string as the key, and the values be the list of potential alternates to try if expected fails.
    /// Returns a Hashmap using the same expected keys as the potential keys hashmap, but the values are the String found (the first matching of each).
    /// Errors if any of the keys fail to extract.
    pub async fn advanced_extract_multi_keys_from_inference_response(
        agent: SerializedAgent,
        response: LLMInferenceResponse,
        filled_prompt: Prompt,
        potential_keys_hashmap: HashMap<&str, Vec<&str>>,
        retry_attempts: u64,
    ) -> Result<HashMap<String, String>, AgentError> {
        let (value, _) = JobManager::advanced_extract_multi_keys_from_inference_response_with_json(
            agent.clone(),
            response.clone(),
            filled_prompt.clone(),
            potential_keys_hashmap.clone(),
            retry_attempts.clone(),
        )
        .await?;

        Ok(value)
    }

    /// Attempts to extract multiple keys from the inference response, including retry inferencing/upper + lower if necessary.
    /// Potential keys hashmap should have the expected string as the key, and the values be the list of potential alternates to try if expected fails.
    /// Returns a Hashmap using the same expected keys as the potential keys hashmap, but the values are the String found (the first matching of each).
    /// Also returns the response result (which will be new if at least one inference retry was done).
    /// Errors if any of the keys fail to extract.
    pub async fn advanced_extract_multi_keys_from_inference_response_with_json(
        agent: SerializedAgent,
        response: LLMInferenceResponse,
        filled_prompt: Prompt,
        potential_keys_hashmap: HashMap<&str, Vec<&str>>,
        retry_attempts: u64,
    ) -> Result<(HashMap<String, String>, LLMInferenceResponse), AgentError> {
        let mut result_map = HashMap::new();
        let mut new_response = response.clone();

        for (key, potential_keys) in potential_keys_hashmap {
            let (value, res) = JobManager::advanced_extract_key_from_inference_response_with_new_response(
                agent.clone(),
                response.clone(),
                filled_prompt.clone(),
                potential_keys.iter().map(|k| k.to_string()).collect(),
                retry_attempts.clone(),
            )
            .await?;
            result_map.insert(key.to_string(), value);
            new_response = res;
        }
        return Ok((result_map, new_response));
    }

    /// Attempts to extract a single key from the inference response (first matched of potential_keys), including retry inferencing if necessary.
    /// Also tries variants of each potential key using capitalization/casing.
    /// Returns the String found at the first matching key.
    pub async fn advanced_extract_key_from_inference_response(
        agent: SerializedAgent,
        response: LLMInferenceResponse,
        filled_prompt: Prompt,
        potential_keys: Vec<String>,
        retry_attempts: u64,
    ) -> Result<String, AgentError> {
        let (value, _) = JobManager::advanced_extract_key_from_inference_response_with_new_response(
            agent.clone(),
            response.clone(),
            filled_prompt.clone(),
            potential_keys.clone(),
            retry_attempts.clone(),
        )
        .await?;

        Ok(value)
    }

    /// Attempts to extract a single key from the inference response (first matched of potential_keys), including retry inferencing if necessary.
    /// Also tries variants of each potential key using capitalization/casing.
    /// Returns a tuple of the String found at the first matching key + the (potentially new) response markdown parsed to JSON (new if retry was done).
    pub async fn advanced_extract_key_from_inference_response_with_new_response(
        agent: SerializedAgent,
        response: LLMInferenceResponse,
        filled_prompt: Prompt,
        potential_keys: Vec<String>,
        retry_attempts: u64,
    ) -> Result<(String, LLMInferenceResponse), AgentError> {
        if potential_keys.is_empty() {
            return Err(AgentError::InferenceJSONResponseMissingField(
                "No keys supplied to attempt to extract".to_string(),
            ));
        }

        for key in &potential_keys {
            if let Ok(value) = JobManager::direct_extract_key_inference_response(response.clone(), key) {
                return Ok((value, response));
            }
        }

        let mut current_response = response.original_response_string;
        for _ in 0..retry_attempts {
            for key in &potential_keys {
                let new_response = internal_fix_markdown_to_include_proper_key(
                    agent.clone(),
                    current_response.to_string(),
                    filled_prompt.clone(),
                    key.to_string(),
                )
                .await?;
                if let Ok(value) = JobManager::direct_extract_key_inference_response(new_response.clone(), key) {
                    return Ok((value, new_response.clone()));
                }
                current_response = new_response.original_response_string;
            }
        }

        Err(AgentError::InferenceJSONResponseMissingField(potential_keys.join(", ")))
    }

    /// Attempts to extract a String using the provided key in the JSON response.
    /// Also tries variants of the provided key using capitalization/casing.
    pub fn direct_extract_key_inference_response(
        response: LLMInferenceResponse,
        key: &str,
    ) -> Result<String, AgentError> {
        let response_json = response.json;
        let keys_to_try = [
            key.to_string(),
            key[..1].to_uppercase() + &key[1..],
            key.to_uppercase(),
            key.to_lowercase(),
            to_snake_case(key),
            to_camel_case(key),
            to_dash_case(key),
        ];

        for key_variant in keys_to_try.iter() {
            if let Some(value) = response_json.get(key_variant) {
                let value_str = match value {
                    JsonValue::String(s) => s.clone(),
                    _ => value.to_string(),
                };
                return Ok(value_str);
            }
        }

        Err(AgentError::InferenceJSONResponseMissingField(key.to_string()))
    }

    /// Inferences the Agent's LLM with the given markdown prompt. Automatically validates the response is
    /// a valid markdown, and processes it into a json.
    pub async fn inference_agent_markdown(
        agent: SerializedAgent,
        filled_prompt: Prompt,
    ) -> Result<LLMInferenceResponse, AgentError> {
        let agent_cloned = agent.clone();
        let prompt_cloned = filled_prompt.clone();
        let task_response = tokio::spawn(async move {
            let agent = Agent::from_serialized_agent(agent_cloned);
            agent.inference_markdown(prompt_cloned).await
        })
        .await;

        let response = task_response?;
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("inference_agent_markdown> response: {:?}", response).as_str(),
        );

        response
    }

    /// Fetches boilerplate/relevant data required for a job to process a step
    /// it may return an outdated node_name
    pub async fn fetch_relevant_job_data(
        job_id: &str,
        db: Arc<ShinkaiDB>,
    ) -> Result<(Job, Option<SerializedAgent>, String, Option<ShinkaiName>), AgentError> {
        // Fetch the job
        let full_job = { db.get_job(job_id)? };

        // Acquire Agent
        let agent_id = full_job.parent_agent_id.clone();
        let mut agent_found = None;
        let mut profile_name = String::new();
        let mut user_profile: Option<ShinkaiName> = None;
        let agents = JobManager::get_all_agents(db).await.unwrap_or(vec![]);
        for agent in agents {
            if agent.id == agent_id {
                agent_found = Some(agent.clone());
                profile_name = agent.full_identity_name.full_name.clone();
                user_profile = Some(agent.full_identity_name.extract_profile().unwrap());
                break;
            }
        }

        Ok((full_job, agent_found, profile_name, user_profile))
    }

    pub async fn get_all_agents(db: Arc<ShinkaiDB>) -> Result<Vec<SerializedAgent>, ShinkaiDBError> {
        db.get_all_agents()
    }

    /// Converts the values of the inference response json, into strings to work nicely with
    /// rest of the stack
    pub fn convert_inference_response_to_internal_strings(value: JsonValue) -> JsonValue {
        match value {
            JsonValue::String(s) => JsonValue::String(s.clone()),
            JsonValue::Array(arr) => JsonValue::String(
                arr.iter()
                    .map(|v| match v {
                        JsonValue::String(s) => format!("- {}", s),
                        _ => format!("- {}", v),
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            ),
            JsonValue::Object(obj) => {
                let mut res = Map::new();
                for (k, v) in obj {
                    res.insert(k.clone(), JobManager::convert_inference_response_to_internal_strings(v));
                }
                JsonValue::Object(res)
            }
            _ => JsonValue::String(value.to_string()),
        }
    }
}

// Helper function to convert a string to snake_case
fn to_snake_case(s: &str) -> String {
    s.chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() {
                if i == 0 {
                    c.to_lowercase().to_string()
                } else {
                    format!("_{}", c.to_lowercase())
                }
            } else {
                c.to_string()
            }
        })
        .collect()
}

// Helper function to convert a string to camelCase
fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut uppercase_next = false;
    for c in s.chars() {
        if c == '_' {
            uppercase_next = true;
        } else if uppercase_next {
            result.push(c.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

// Helper function to convert a string to dash-case (kebab-case)
fn to_dash_case(s: &str) -> String {
    s.chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() {
                if i == 0 {
                    c.to_lowercase().to_string()
                } else {
                    format!("-{}", c.to_lowercase())
                }
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// Inferences the LLM again asking it to take its previous answer and make sure it responds with markdown and include the required key.
async fn internal_fix_markdown_to_include_proper_key(
    agent: SerializedAgent,
    invalid_markdown: String,
    original_prompt: Prompt,
    key_to_correct: String,
) -> Result<LLMInferenceResponse, AgentError> {
    let response = tokio::spawn(async move {
        let agent = Agent::from_serialized_agent(agent);
        let prompt = JobPromptGenerator::basic_fix_markdown_to_include_proper_key(
            invalid_markdown,
            original_prompt,
            key_to_correct,
        );
        eprintln!(
            "!?! Attempting to fix markdown. Re-inferencing: {:?}",
            prompt.sub_prompts
        );
        agent.inference_markdown(prompt).await
    })
    .await;
    let response = match response {
        Ok(res) => res?,
        Err(e) => {
            eprintln!("Task panicked with error: {:?}", e);
            return Err(AgentError::InferenceFailed);
        }
    };

    Ok(response)
}
