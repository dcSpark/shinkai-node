use crate::agent::agent::Agent;
use crate::agent::error::AgentError;
use crate::agent::job::Job;
use crate::agent::job_manager::AgentManager;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

pub enum InferenceChain {
    QAChain,
    ToolExecutionChain,
    CodingChain,
}

impl AgentManager {
    /// Chooses an inference chain based on the job message (using the agent's LLM)
    /// and then starts using the chosen chain.
    /// Returns the final String result from the inferencing, and a new execution context.
    pub async fn inference_chain_router(
        &self,
        agent_found: Option<Arc<Mutex<Agent>>>,
        full_job: Job,
        job_message: JobMessage,
        prev_execution_context: HashMap<String, String>,
        generator: &dyn EmbeddingGenerator,
        user_profile: Option<ShinkaiName>,
    ) -> Result<(String, HashMap<String, String>), AgentError> {
        // TODO: Later implement inference chain decision making here before choosing which chain to use.
        // For now we just use qa inference chain by default.
        let chosen_chain = InferenceChain::QAChain;
        let mut inference_response_content = String::new();
        let mut new_execution_context = HashMap::new();

        match chosen_chain {
            InferenceChain::QAChain => {
                if let Some(agent) = agent_found {
                    inference_response_content = self
                        .process_qa_inference_chain(
                            full_job,
                            job_message.content.clone(),
                            agent,
                            prev_execution_context,
                            generator,
                            user_profile,
                            None,
                            Some(job_message.content.clone()),
                            None,
                            0,
                        )
                        .await?;
                    new_execution_context
                        .insert("previous_step_response".to_string(), inference_response_content.clone());
                } else {
                    return Err(AgentError::AgentNotFound);
                }
            }
            // Add other chains here
            _ => {}
        };

        Ok((inference_response_content, new_execution_context))
    }
}
