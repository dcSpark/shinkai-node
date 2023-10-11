use crate::agent::agent::Agent;
use crate::agent::error::AgentError;
use crate::agent::execution::job_prompts::JobPromptGenerator;
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
        prev_search_text: Option<String>,
        summary_text: Option<String>,
        iteration_count: u64,
    ) -> Result<String, AgentError> {
        println!("start_qa_inference_chain>  message: {:?}", job_task);

        // Use search_text if available (on recursion), otherwise use job_task to generate the query (on first iteration)
        let query_text = search_text.clone().unwrap_or(job_task.clone());
        let query = generator.generate_embedding_default(&query_text).unwrap();
        let ret_data_chunks = self
            .job_scope_vector_search(full_job.scope(), query, 20, &user_profile.clone().unwrap())
            .await?;

        // Use the default prompt if not reached final iteration count, else use final prompt
        let filled_prompt = if iteration_count < 5 {
            JobPromptGenerator::response_prompt_with_vector_search(
                job_task.clone(),
                ret_data_chunks,
                summary_text,
                prev_search_text,
            )
        } else {
            JobPromptGenerator::response_prompt_with_vector_search_final(
                job_task.clone(),
                ret_data_chunks,
                summary_text,
            )
        };

        // Inference the agent's LLM with the prompt
        let response_json = self.inference_agent(agent.clone(), filled_prompt).await?;

        // If it has an answer, the chain is finished and so just return the answer response as a String
        if let Some(answer) = response_json.get("answer") {
            let answer_str = answer
                .as_str()
                .ok_or_else(|| AgentError::InferenceJSONResponseMissingField("answer".to_string()))?;
            let cleaned_answer = Self::ending_stripper(&answer_str);
            println!("QA Chain Final Answer: {:?}", cleaned_answer);
            return Ok(cleaned_answer);
        }
        // If iteration_count is > 5 and we still don't have an answer, return an error
        else if iteration_count > 5 {
            return Err(AgentError::InferenceRecursionLimitReached(job_task.clone()));
        }

        // If not an answer, then the LLM must respond with a search/summary, so we parse them
        // to use for the next recursive call
        let (new_search_text, summary) = match response_json.get("search") {
            Some(search) => {
                let search_str = search
                    .as_str()
                    .ok_or_else(|| AgentError::InferenceJSONResponseMissingField("search".to_string()))?;
                let summary_str = response_json
                    .get("summary")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string());
                (search_str, summary_str)
            }
            None => return Err(AgentError::InferenceJSONResponseMissingField("search".to_string())),
        };

        // Recurse with the new search/summary text and increment iteration_count
        self.start_qa_inference_chain(
            full_job,
            job_task.to_string(),
            agent,
            execution_context,
            generator,
            user_profile,
            Some(new_search_text.to_string()),
            search_text,
            summary,
            iteration_count + 1,
        )
        .await
    }

    /// Removes last sentence from answer if it contains any of the unwanted phrases.
    /// This is used because the LLM sometimes answers properly, but then adds useless last sentence such as
    /// "However, specific details are not provided in the content." at the end.
    pub fn ending_stripper(answer: &str) -> String {
        let mut sentences: Vec<&str> = answer.split('.').collect();

        let unwanted_phrases = [
            "however",
            "unfortunately",
            "additional research",
            "may be required",
            "i do not",
            "further information",
            "specific details",
            "provided content",
            "not available",
        ];

        while let Some(last_sentence) = sentences.pop() {
            if last_sentence.trim().is_empty() {
                continue;
            }
            let sentence = last_sentence.trim_start().to_lowercase();
            if !unwanted_phrases.iter().any(|&phrase| sentence.contains(phrase)) {
                sentences.push(last_sentence);
            }
            break;
        }

        sentences.join(".")
    }
}
