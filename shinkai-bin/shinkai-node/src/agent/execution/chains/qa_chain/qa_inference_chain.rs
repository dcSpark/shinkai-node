use crate::agent::error::AgentError;
use crate::agent::execution::chains::inference_chain_trait::{
    InferenceChain, InferenceChainContext, InferenceChainResult, LLMInferenceResponse,
};
use crate::agent::execution::prompts::prompts::JobPromptGenerator;
use crate::agent::job::{Job, JobLike};
use crate::agent::job_manager::JobManager;
use crate::db::ShinkaiDB;
use crate::vector_fs::vector_fs::VectorFS;
use async_recursion::async_recursion;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::vector_resource::RetrievedNode;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct QAInferenceChain {
    pub context: InferenceChainContext,
    pub search_text: Option<String>,
    pub summary_text: Option<String>,
}

#[async_trait]
impl InferenceChain for QAInferenceChain {
    fn chain_id() -> String {
        "qa_inference_chain".to_string()
    }

    fn chain_context(&mut self) -> &mut InferenceChainContext {
        &mut self.context
    }

    async fn run_chain(&mut self) -> Result<InferenceChainResult, AgentError> {
        let response = QAInferenceChain::start_qa_inference_chain(
            self.context.db.clone(),
            self.context.vector_fs.clone(),
            self.context.full_job.clone(),
            self.context.user_message.original_user_message_string.to_string(),
            self.context.agent.clone(),
            self.context.execution_context.clone(),
            self.context.generator.clone(),
            self.context.user_profile.clone(),
            self.search_text.clone(),
            self.summary_text.clone(),
            0,
            self.context.max_iterations,
            self.context.max_tokens_in_prompt,
        )
        .await?;
        let job_execution_context = self.context.execution_context.clone();
        Ok(InferenceChainResult::new(response, job_execution_context))
    }
}

impl QAInferenceChain {
    pub fn new(context: InferenceChainContext) -> Self {
        Self {
            context,
            search_text: None,
            summary_text: None,
        }
    }

    /// An inference chain for question-answer user messages which vector searches the Vector Resources
    /// in the JobScope to find relevant content for the LLM to use at each step.
    #[async_recursion]
    #[instrument(skip(generator, vector_fs, db))]
    #[allow(clippy::too_many_arguments)]
    pub async fn start_qa_inference_chain(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        user_message: String,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        search_text: Option<String>,
        summary_text: Option<String>,
        iteration_count: u64,
        max_iterations: u64,
        max_tokens_in_prompt: usize,
    ) -> Result<String, AgentError> {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("start_qa_inference_chain>  message: {:?}", user_message),
        );

        //
        // TODO: If the job scope has least 1 VectorFS folder or at least 4 VRs, then add new logic only on first iteration of this chain.
        // Instead of just doing keyword extraction, do an LLM inference and ask it to return a json list of strings to use for vector search (up to 10 maybe?)
        // to be able to find all the information related to the user's questions. This will likely be needed when lots of VRs are part of the scope.
        //

        // Use search_text if available (on recursion), otherwise use user_message to generate the query (on first iteration)
        let query_text = search_text.clone().unwrap_or(user_message.clone());

        // Vector Search if the scope isn't empty.
        let scope_is_empty = full_job.scope().is_empty();
        let mut ret_nodes: Vec<RetrievedNode> = vec![];
        let mut summary_node_text = None;
        if !scope_is_empty {
            let (ret, summary) = JobManager::keyword_chained_job_scope_vector_search(
                db.clone(),
                vector_fs.clone(),
                full_job.scope(),
                query_text.clone(),
                &user_profile,
                generator.clone(),
                20,
                max_tokens_in_prompt,
            )
            .await?;
            ret_nodes = ret;
            summary_node_text = summary;
        }

        // Use the default prompt if not reached final iteration count, else use final prompt
        let is_not_final = iteration_count < max_iterations && !scope_is_empty;
        let filled_prompt = if is_not_final {
            JobPromptGenerator::qa_response_prompt_with_vector_search(
                user_message.clone(),
                ret_nodes,
                summary_text.clone(),
                Some(query_text),
                Some(full_job.step_history.clone()),
            )
        } else {
            JobPromptGenerator::qa_response_prompt_with_vector_search_final(
                user_message.clone(),
                ret_nodes,
                summary_text.clone(),
                Some(full_job.step_history.clone()),
                iteration_count,
                max_tokens_in_prompt,
            )
        };

        // Inference the agent's LLM with the prompt
        let response_res = JobManager::inference_agent_markdown(agent.clone(), filled_prompt.clone()).await;
        // Check if it failed to produce a proper json object at all, and if so go through more advanced retry logic

        if let Err(AgentError::LLMProviderInferenceLimitReached(e)) = &response_res {
            return Err(AgentError::LLMProviderInferenceLimitReached(e.to_string()));
        } else if let Err(AgentError::LLMProviderUnexpectedError(e)) = &response_res {
            return Err(AgentError::LLMProviderUnexpectedError(e.to_string()));
        } else if response_res.is_err() {
            return no_json_object_retry_logic(
                response_res,
                db,
                vector_fs,
                full_job,
                user_message,
                agent,
                execution_context,
                generator,
                user_profile,
                summary_text,
                summary_node_text,
                iteration_count,
                max_iterations,
                max_tokens_in_prompt,
            )
            .await;
        }

        // Extract the JSON from the inference response Result and proceed forward
        let response = response_res?;
        let answer = JobManager::direct_extract_key_inference_response(response.clone(), "answer");

        // If it has an answer, the chain is finished and so just return the answer response as a cleaned String
        if let Ok(answer_str) = answer {
            return Ok(answer_str);
        }
        // If it errored and past max iterations, try to use the summary from the previous iteration, or return error
        else if let Err(_) = answer {
            if iteration_count > max_iterations {
                if let Some(summary_str) = &summary_text {
                    return Ok(summary_str.to_string());
                } else {
                    return Err(AgentError::InferenceRecursionLimitReached(user_message.clone()));
                }
            }
        }

        // If not an answer, then the LLM must respond with a search/summary, so we parse them
        // to use for the next recursive call
        let (new_search_text, summary) =
            match &JobManager::advanced_extract_key_from_inference_response_with_new_response(
                agent.clone(),
                response.clone(),
                filled_prompt.clone(),
                vec!["summary".to_string(), "answer".to_string(), "text".to_string()],
                3,
            )
            .await
            {
                Ok((summary_str, new_resp_json)) => {
                    let new_search_text =
                        match &JobManager::advanced_extract_key_from_inference_response_with_new_response(
                            agent.clone(),
                            new_resp_json.clone(),
                            filled_prompt.clone(),
                            vec!["search".to_string(), "lookup".to_string()],
                            2,
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
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Error,
                        &format!(
                            "Failed qa inference chain: Missing Field {}. Responding while whole LLM markdown response",
                            "summary"
                        ),
                    );
                    // return Err(AgentError::InferenceJSONResponseMissingField("summary".to_string()));
                    return Ok(response.original_response_string);
                }
            };

                // TODO: Local LLMs struggle to summarize AND decide if it's enough content, it should be a consecutive process to avoid this issue
                Ok(summary)

        // If the new search text is the same as the previous one, prompt the agent for a new search term
        // let mut new_search_text = new_search_text.clone();
        // if Some(new_search_text.clone()) == search_text && !full_job.scope.is_empty() {
        //     let retry_prompt =
        //         JobPromptGenerator::retry_new_search_term_prompt(new_search_text.clone(), summary.clone());
        //     let response = JobManager::inference_agent_markdown(agent.clone(), retry_prompt).await;
        //     if let Ok(response) = response {
        //         match JobManager::direct_extract_key_inference_response(response, "search") {
        //             Ok(search_str) => {
        //                 new_search_text = search_str;
        //             }
        //             // If extracting fails, use summary to make the new search text likely different compared to last iteration
        //             Err(_) => new_search_text.clone_from(&summary),
        //         }
        //     } else {
        //         new_search_text.clone_from(&summary);
        //     }
        // }

        // // Recurse with the new search/summary text and increment iteration_count
        // QAInferenceChain::start_qa_inference_chain(
        //     db,
        //     vector_fs,
        //     full_job,
        //     user_message.to_string(),
        //     agent,
        //     execution_context,
        //     generator,
        //     user_profile,
        //     Some(new_search_text),
        //     Some(summary.to_string()),
        //     iteration_count + 1,
        //     max_iterations,
        //     max_tokens_in_prompt,
        // )
        // .await
    }
}

#[allow(clippy::too_many_arguments)]
async fn no_json_object_retry_logic(
    response: Result<LLMInferenceResponse, AgentError>,
    db: Arc<ShinkaiDB>,
    vector_fs: Arc<VectorFS>,
    full_job: Job,
    user_message: String,
    agent: SerializedAgent,
    execution_context: HashMap<String, String>,
    generator: RemoteEmbeddingGenerator,
    user_profile: ShinkaiName,
    summary_text: Option<String>,
    new_summary_node_text: Option<String>,
    iteration_count: u64,
    max_iterations: u64,
    max_tokens_in_prompt: usize,
) -> Result<String, AgentError> {
    if let Err(e) = &response {
        // If still more iterations left, then recurse to try one more time, using summary as the new search text to likely get different LLM output
        if iteration_count < max_iterations {
            return QAInferenceChain::start_qa_inference_chain(
                db,
                vector_fs,
                full_job,
                user_message.to_string(),
                agent,
                execution_context,
                generator,
                user_profile,
                summary_text.clone(),
                summary_text,
                iteration_count + 1,
                max_iterations,
                max_tokens_in_prompt,
            )
            .await;
        }
        // Else if we're past the max iterations, return either last valid summary from previous iterations or VR summary
        else {
            shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    &format!("Qa inference chain failure due to no parsable JSON produced: {}\nUsing summary backup to respond to user.", e),
                );
            // Try from previous iteration
            let mut summary_answer = String::new();
            if let Some(summary_str) = &summary_text {
                if summary_str.len() > 2 {
                    summary_answer = summary_str.to_string();
                } else {
                    // This propagates the error upwards
                    response?;
                }
            }
            // Else use the VR summary.
            else {
                let mut _temp_resp = JsonValue::Null;
                if let Some(text) = new_summary_node_text {
                    if text.len() > 2 {
                        summary_answer = text.to_string();
                    } else {
                        response?;
                    }
                } else {
                    response?;
                }
            }

            // Return the summary
            return Ok(summary_answer);
        }
    }
    Err(AgentError::InferenceFailed)
}
