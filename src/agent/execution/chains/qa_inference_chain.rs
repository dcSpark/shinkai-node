use crate::agent::error::AgentError;
use crate::agent::execution::job_prompts::JobPromptGenerator;
use crate::agent::file_parsing::ParsingHelper;
use crate::agent::job::{Job, JobId, JobLike};
use crate::agent::job_manager::JobManager;
use crate::db::ShinkaiDB;
use crate::vector_fs::vector_fs::VectorFS;
use async_recursion::async_recursion;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::instrument;

impl JobManager {
    /// An inference chain for question-answer job tasks which vector searches the Vector Resources
    /// in the JobScope to find relevant content for the LLM to use at each step.
    #[async_recursion]
    #[instrument(skip(generator, vector_fs, db))]
    pub async fn start_qa_inference_chain(
        db: Arc<Mutex<ShinkaiDB>>,
        vector_fs: Arc<Mutex<VectorFS>>,
        full_job: Job,
        job_task: String,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        generator: &dyn EmbeddingGenerator,
        user_profile: ShinkaiName,
        search_text: Option<String>,
        summary_text: Option<String>,
        iteration_count: u64,
        max_iterations: u64,
    ) -> Result<String, AgentError> {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("start_qa_inference_chain>  message: {:?}", job_task),
        );

        // Use search_text if available (on recursion), otherwise use job_task to generate the query (on first iteration)
        let query_text = search_text.clone().unwrap_or(job_task.clone());
        let query = generator.generate_embedding_default(&query_text).await?;
        let ret_nodes = JobManager::job_scope_vector_search(
            db.clone(),
            vector_fs.clone(),
            full_job.scope(),
            query,
            query_text.clone(),
            20,
            &user_profile,
            true,
        )
        .await?;
        // Text from the first node, which is the summary of the most similar VR
        let summary_node_text = ret_nodes
            .get(0)
            .and_then(|node| node.node.get_text_content().ok())
            .map(|text| text.to_string());

        // Use the default prompt if not reached final iteration count, else use final prompt
        let is_not_final = iteration_count < max_iterations && !full_job.scope.is_empty();
        let filled_prompt = if is_not_final {
            JobPromptGenerator::response_prompt_with_vector_search(
                job_task.clone(),
                ret_nodes,
                summary_text.clone(),
                Some(query_text),
                Some(full_job.step_history.clone()),
            )
        } else {
            JobPromptGenerator::response_prompt_with_vector_search_final(
                job_task.clone(),
                ret_nodes,
                summary_text.clone(),
                Some(full_job.step_history.clone()),
            )
        };

        // Inference the agent's LLM with the prompt, and check if it failed to produce a proper json object at all
        let response = JobManager::inference_agent(agent.clone(), filled_prompt.clone()).await;
        if let Err(e) = &response {
            // If still more iterations left, then recurse to try one more time, using summary as the new search text to likely get different LLM output
            if iteration_count < max_iterations {
                return JobManager::start_qa_inference_chain(
                    db,
                    vector_fs,
                    full_job,
                    job_task.to_string(),
                    agent,
                    execution_context,
                    generator,
                    user_profile,
                    summary_text.clone(),
                    summary_text,
                    iteration_count + 1,
                    max_iterations,
                )
                .await;
            }
            // Else if we're past the max iterations, return either last valid summary from previous iterations or VR summary
            else {
                eprintln!("Qa inference chain failure due to no parsable JSON produced: {}\nUsing summary backup to respond to user.", e);
                let mut summary_answer = String::new();
                // Try from previous iteration
                if let Some(summary_str) = &summary_text {
                    summary_answer = summary_str.to_string()
                }
                // Else use the VR summary. We create _temp_res to have `response?` resolve to pushing the error properly
                else {
                    let mut _temp_resp = JsonValue::Null;
                    match summary_node_text {
                        Some(text) => summary_answer = text.to_string(),
                        None => _temp_resp = response?,
                    }
                }

                let cleaned_answer =
                    ParsingHelper::flatten_to_content_if_json(&ParsingHelper::ending_stripper(summary_answer.as_str()));
                return Ok(cleaned_answer);
            }
        }

        // Attempt to extract the JSON from the Result and proceed forward. This is where method will fail if after everything LLM never produced a single valid summary
        let response_json = response?;
        let answer = JobManager::direct_extract_key_inference_json_response(response_json.clone(), "answer");

        // If it has an answer, the chain is finished and so just return the answer response as a cleaned String
        if let Ok(answer_str) = answer {
            let cleaned_answer =
                ParsingHelper::flatten_to_content_if_json(&ParsingHelper::ending_stripper(&answer_str));
            return Ok(cleaned_answer);
        }
        // If it errored and past max iterations, try to use the summary from the previous iteration, or return error
        else if let Err(e) = answer {
            if iteration_count > max_iterations {
                if let Some(summary_str) = &summary_text {
                    let cleaned_answer = ParsingHelper::flatten_to_content_if_json(&ParsingHelper::ending_stripper(
                        summary_str.as_str(),
                    ));
                    return Ok(cleaned_answer);
                } else {
                    eprintln!("Failed qa inference chain: {}", e);
                    return Err(AgentError::InferenceRecursionLimitReached(job_task.clone()));
                }
            }
        }

        // If not an answer, then the LLM must respond with a search/summary, so we parse them
        // to use for the next recursive call
        let (new_search_text, summary) = match &JobManager::advanced_extract_key_from_inference_response(
            agent.clone(),
            response_json.clone(),
            filled_prompt.clone(),
            vec!["summary".to_string(), "answer".to_string()],
            2,
        )
        .await
        {
            Ok((summary_str, new_resp_json)) => {
                let new_search_text = match &JobManager::advanced_extract_key_from_inference_response(
                    agent.clone(),
                    new_resp_json.clone(),
                    filled_prompt.clone(),
                    vec!["search".to_string(), "lookup".to_string()],
                    4,
                )
                .await
                {
                    Ok((search_text, _)) => Some(search_text.to_string()),
                    Err(_) => None,
                };
                // Just use summary string as search text if LLM didn't provide one to decease # of inferences
                (
                    new_search_text.unwrap_or(summary_str.to_string()),
                    summary_str.to_string(),
                )
            }
            Err(_) => {
                eprintln!("Failed qa inference chain: Missing Field {}", "search");
                return Err(AgentError::InferenceJSONResponseMissingField("search".to_string()));
            }
        };

        // If the new search text is the same as the previous one, prompt the agent for a new search term
        let mut new_search_text = new_search_text.clone();
        if Some(new_search_text.clone()) == search_text && !full_job.scope.is_empty() {
            let retry_prompt =
                JobPromptGenerator::retry_new_search_term_prompt(new_search_text.clone(), summary.clone());
            let response_json = JobManager::inference_agent(agent.clone(), retry_prompt).await?;
            match JobManager::direct_extract_key_inference_json_response(response_json, "search") {
                Ok(search_str) => {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Info,
                        &format!("QA Chain New Search Retry Term: {:?}", search_str),
                    );
                    new_search_text = search_str;
                }
                // Use summary to make the new search text likely different compared to last iteration
                Err(_) => new_search_text = summary.clone(),
            }
        }

        // Recurse with the new search/summary text and increment iteration_count
        JobManager::start_qa_inference_chain(
            db,
            vector_fs,
            full_job,
            job_task.to_string(),
            agent,
            execution_context,
            generator,
            user_profile,
            Some(new_search_text),
            Some(summary.to_string()),
            iteration_count + 1,
            max_iterations,
        )
        .await
    }
}
