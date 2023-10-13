use crate::agent::agent::Agent;
use crate::agent::error::AgentError;
use crate::agent::execution::job_prompts::JobPromptGenerator;
use crate::agent::file_parsing::ParsingHelper;
use crate::agent::job::{Job, JobId, JobLike};
use crate::agent::job_manager::AgentManager;
use async_recursion::async_recursion;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

impl AgentManager {
    /// An inference chain for question-answer job tasks which vector searches the Vector Resources
    /// in the JobScope to find relevant content for the LLM to use at each step.
    #[async_recursion]
    pub async fn start_qa_inference_chain(
        &self,
        full_job: Job,
        job_task: String,
        agent: Arc<Mutex<Agent>>,
        execution_context: HashMap<String, String>,
        generator: &dyn EmbeddingGenerator,
        user_profile: Option<ShinkaiName>,
        search_text: Option<String>,
        summary_text: Option<String>,
        iteration_count: u64,
        max_iterations: u64,
    ) -> Result<String, AgentError> {
        println!("start_qa_inference_chain>  message: {:?}", job_task);

        // Use search_text if available (on recursion), otherwise use job_task to generate the query (on first iteration)
        let query_text = search_text.clone().unwrap_or(job_task.clone());
        let query = generator.generate_embedding_default(&query_text).unwrap();
        let ret_data_chunks = self
            .job_scope_vector_search(full_job.scope(), query, 20, &user_profile.clone().unwrap(), true)
            .await?;

        // Use the default prompt if not reached final iteration count, else use final prompt
        let filled_prompt = if iteration_count < max_iterations {
            // Response from the previous job step
            let previous_job_step_response = if iteration_count == 0 {
                execution_context.get("previous_step_response").cloned()
            } else {
                None
            };
            JobPromptGenerator::response_prompt_with_vector_search(
                job_task.clone(),
                ret_data_chunks,
                summary_text,
                Some(query_text),
                previous_job_step_response,
            )
        } else {
            JobPromptGenerator::response_prompt_with_vector_search_final(
                job_task.clone(),
                ret_data_chunks,
                summary_text,
            )
        };

        // Inference the agent's LLM with the prompt. If it has an answer, the chain
        // is finished and so just return the answer response as a cleaned String
        let response_json = self.inference_agent(agent.clone(), filled_prompt).await?;
        if let Ok(answer_str) = self.extract_inference_json_response(response_json.clone(), "answer") {
            let cleaned_answer = ParsingHelper::ending_stripper(&answer_str);
            println!("QA Chain Final Answer: {:?}", cleaned_answer);
            return Ok(cleaned_answer);
        }
        // If iteration_count is > max_iterations and we still don't have an answer, return an error
        else if iteration_count > max_iterations {
            return Err(AgentError::InferenceRecursionLimitReached(job_task.clone()));
        }

        // If not an answer, then the LLM must respond with a search/summary, so we parse them
        // to use for the next recursive call
        let (mut new_search_text, summary) = match self.extract_inference_json_response(response_json.clone(), "search")
        {
            Ok(search_str) => {
                let summary_str = response_json
                    .get("summary")
                    .and_then(|s| s.as_str())
                    .map(|s| ParsingHelper::ending_stripper(s));
                (search_str, summary_str)
            }
            Err(_) => return Err(AgentError::InferenceJSONResponseMissingField("search".to_string())),
        };

        // If the new search text is the same as the previous one, prompt the agent for a new search term
        if Some(new_search_text.clone()) == search_text {
            let retry_prompt = JobPromptGenerator::retry_new_search_term_prompt(
                new_search_text.clone(),
                summary.clone().unwrap_or_default(),
            );
            let response_json = self.inference_agent(agent.clone(), retry_prompt).await?;
            match self.extract_inference_json_response(response_json, "search") {
                Ok(search_str) => {
                    println!("QA Chain New Search Retry Term: {:?}", search_str);
                    new_search_text = search_str;
                }
                Err(_) => {}
            }
        }

        // Recurse with the new search/summary text and increment iteration_count
        self.start_qa_inference_chain(
            full_job,
            job_task.to_string(),
            agent,
            execution_context,
            generator,
            user_profile,
            Some(new_search_text),
            summary,
            iteration_count + 1,
            max_iterations,
        )
        .await
    }
}
