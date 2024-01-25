use super::error::AgentError;
use super::execution::job_prompts::Prompt;
use super::providers::LLMProvider;
use reqwest::Client;
use serde_json::{Map, Value as JsonValue};
use shinkai_message_primitives::{
    schemas::{
        agents::serialized_agent::{AgentLLMInterface, SerializedAgent},
        shinkai_name::ShinkaiName,
    },
    shinkai_message::shinkai_message_schemas::{JobPreMessage, JobRecipient},
};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub full_identity_name: ShinkaiName,
    pub client: Client,
    pub perform_locally: bool,        // Todo: Remove as not used anymore
    pub external_url: Option<String>, // external API URL
    pub api_key: Option<String>,
    pub model: AgentLLMInterface,
    pub toolkit_permissions: Vec<String>,        // Todo: remove as not used
    pub storage_bucket_permissions: Vec<String>, // Todo: remove as not used
    pub allowed_message_senders: Vec<String>,    // list of sub-identities allowed to message the agent
}

impl Agent {
    pub fn new(
        id: String,
        full_identity_name: ShinkaiName,
        perform_locally: bool,
        external_url: Option<String>,
        api_key: Option<String>,
        model: AgentLLMInterface,
        toolkit_permissions: Vec<String>,
        storage_bucket_permissions: Vec<String>,
        allowed_message_senders: Vec<String>,
    ) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap();
        Self {
            id,
            full_identity_name,
            client,
            perform_locally,
            external_url,
            api_key,
            model,
            toolkit_permissions,
            storage_bucket_permissions,
            allowed_message_senders,
        }
    }

    /// Inferences an LLM locally based on info held in the Agent
    /// TODO: For now just mocked, eventually get around to this, and create a struct that implements the Provider trait to unify local with remote interface.
    async fn inference_locally(&self, content: String) -> Result<JsonValue, AgentError> {
        // Here we run our GPU-intensive task on a separate thread
        let handle = tokio::task::spawn_blocking(move || {
            let mut map = Map::new();
            map.insert(
                "answer".to_string(),
                JsonValue::String("\n\nHello there, how may I assist you today?".to_string()),
            );
            JsonValue::Object(map)
        });

        match handle.await {
            Ok(response) => Ok(response),
            Err(e) => Err(AgentError::InferenceFailed),
        }
    }

    /// Inferences the LLM model tied to the agent to get a response back.
    /// Note, all `content` is expected to use prompts from the PromptGenerator,
    /// meaning that they tell/force the LLM to always respond in JSON. We automatically
    /// parse the JSON object out of the response into a JsonValue, or error if no object is found.
    pub async fn inference(&self, prompt: Prompt) -> Result<JsonValue, AgentError> {
        let response = match &self.model {
            AgentLLMInterface::OpenAI(openai) => {
                openai
                    .call_api(&self.client, self.external_url.as_ref(), self.api_key.as_ref(), prompt)
                    .await
            }
            AgentLLMInterface::GenericAPI(genericapi) => {
                genericapi
                    .call_api(&self.client, self.external_url.as_ref(), self.api_key.as_ref(), prompt)
                    .await
            }
            AgentLLMInterface::Ollama(ollama) => {
                ollama
                    .call_api(&self.client, self.external_url.as_ref(), self.api_key.as_ref(), prompt)
                    .await
            }
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => {
                shinkai_backend
                    .call_api(&self.client, self.external_url.as_ref(), self.api_key.as_ref(), prompt)
                    .await
            }
            AgentLLMInterface::LocalLLM(local_llm) => {
                self.inference_locally(prompt.generate_single_output_string()?).await
            }
        }?;

        Ok(cleaned_response)
    }
}

impl Agent {
    pub fn from_serialized_agent(serialized_agent: SerializedAgent) -> Self {
        Self::new(
            serialized_agent.id,
            serialized_agent.full_identity_name,
            serialized_agent.perform_locally,
            serialized_agent.external_url,
            serialized_agent.api_key,
            serialized_agent.model,
            serialized_agent.toolkit_permissions,
            serialized_agent.storage_bucket_permissions,
            serialized_agent.allowed_message_senders,
        )
    }
}

/// Converts the values of the inference response json, into strings to work nicely with
/// rest of the stack
fn clean_inference_response_json(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::String(s) => JsonValue::String(s.clone()),
        JsonValue::Array(arr) => JsonValue::String(
            arr.iter()
                .map(|v| match v {
                    JsonValue::String(s) => format!("- {}", s),
                    _ => format!("- {}", v.to_string()),
                })
                .collect::<Vec<String>>()
                .join("\n"),
        ),
        JsonValue::Object(obj) => {
            let mut res = Map::new();
            for (k, v) in obj {
                res.insert(k.clone(), clean_inference_response_json(v));
            }
            JsonValue::Object(res)
        }
        _ => JsonValue::String(value.to_string()),
    }
}
