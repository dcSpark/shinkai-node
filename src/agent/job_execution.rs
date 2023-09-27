use crate::agent::agent::Agent;
use crate::agent::job::{Job, JobId, JobLike};
use crate::agent::job_prompts::JobPromptGenerator;
use crate::agent::plan_executor::PlanExecutor;
use crate::db::{db_errors::ShinkaiDBError, ShinkaiDB};
use crate::managers::error::JobManagerError;
use crate::managers::job_manager::{AgentManager, JobManager};
use chrono::Utc;
use ed25519_dalek::SecretKey as SignatureStaticKey;
use shinkai_message_primitives::{
    schemas::shinkai_name::{ShinkaiName, ShinkaiNameError},
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{JobCreationInfo, JobMessage, JobPreMessage, MessageSchemaType},
    },
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use std::fmt;
use std::result::Result::Ok;
use std::{collections::HashMap, error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex};

impl AgentManager {
    /// Processes a job message which will trigger a job step
    pub async fn process_job_step(
        &mut self,
        message: ShinkaiMessage,
        job_message: JobMessage,
    ) -> Result<String, JobManagerError> {
        if let Some(job) = self.jobs.lock().await.get(&job_message.job_id) {
            let job = job.clone();
            let mut shinkai_db = self.db.lock().await;
            println!("process_job_step> job_message: {:?}", job_message);
            shinkai_db.add_message_to_job_inbox(&job_message.job_id.clone(), &message)?;

            //
            // Todo: Implement unprocessed messages logic
            // If current unprocessed message count >= 1, then simply add unprocessed message and return success.
            // However if unprocessed message count  == 0, then:
            // 0. You add the unprocessed message to the list in the DB
            // 1. Start a while loop where every time you fetch the unprocessed messages for the job from the DB and check if there's >= 1
            // 2. You read the first/front unprocessed message (not pop from the back)
            // 3. You start analysis phase to generate the execution plan.
            // 4. You then take the execution plan and process the execution phase.
            // 5. Once execution phase succeeds, you then delete the message from the unprocessed list in the DB
            //    and take the result and append it both to the Job inbox and step history
            // 6. As we're in a while loop, go back to 1, meaning any new unprocessed messages added while the step was happening are now processed sequentially

            //
            // let current_unprocessed_message_count = ...
            shinkai_db.add_to_unprocessed_messages_list(job.job_id().to_string(), job_message.content.clone())?;

            std::mem::drop(shinkai_db); // require to avoid deadlock

            let _ = self.analysis_phase(&**job, job_message.clone()).await?;

            // After analysis phase, we execute the resulting execution plan
            //    let executor = PlanExecutor::new(agent, execution_plan)?;
            //    executor.execute_plan();

            return Ok(job_message.job_id.clone());
        } else {
            return Err(JobManagerError::JobNotFound);
        }
    }

    // Begins processing the analysis phase of the job
    pub async fn analysis_phase(&self, job: &dyn JobLike, job_message: JobMessage) -> Result<(), Box<dyn Error>> {
        // Fetch the job
        let job_id = job.job_id().to_string();
        let full_job = { self.db.lock().await.get_job(&job_id).unwrap() };

        // Acquire Agent
        let agent_id = full_job.parent_agent_id.clone();
        let mut agent_found = None;
        for agent in &self.agents {
            let locked_agent = agent.lock().await;
            if locked_agent.id == agent_id {
                agent_found = Some(agent.clone());
                break;
            }
        }

        // Setup initial data to start moving through analysis phase
        let prev_execution_context = full_job.execution_context.clone();
        let analysis_context = HashMap::new();

        // TODO: Later implement all analysis phase chaining/branching logic starting from here
        // and have multiple methods like process_analysis_inference which use different
        // prompts and are called as needed to arrive at a full execution plan ready to be returned

        match agent_found {
            Some(agent) => {
                self.process_analysis_inference(
                    full_job,
                    job_message.content,
                    agent,
                    prev_execution_context,
                    analysis_context,
                )
                .await
            }
            None => Err(Box::new(JobManagerError::AgentNotFound)),
        }
    }

    async fn process_analysis_inference(
        &self,
        job: Job,
        message: String,
        agent: Arc<Mutex<Agent>>,
        execution_context: HashMap<String, String>,
        analysis_context: HashMap<String, String>,
    ) -> Result<(), Box<dyn Error>> {
        // let time_with_comment = format!("{}: {}", "Current datetime ", Utc::now().to_rfc3339());
        println!("analysis_inference>  message: {:?}", message);

        // Generate the needed prompt
        let filled_prompt = JobPromptGenerator::temporary_instant_inference_prompt(message)
            .generate_single_output_string()
            .unwrap();

        // Execute LLM inferencing
        let response = tokio::spawn(async move {
            let mut agent = agent.lock().await;
            agent.inference(filled_prompt).await;
        })
        .await?;
        println!("decision_iteration> response: {:?}", response);

        // TODO: update this fn so it allows for recursion
        // let is_valid = self.is_analysis_phase_output_valid().await;
        // if is_valid == false {
        //     self.decision_iteration(job, context, last_message, agent).await?;
        // }

        Ok(())
    }

    async fn is_analysis_phase_output_valid(&self) -> bool {
        // Check if the output is valid
        // If not valid, return false
        // If valid, return true
        unimplemented!()
    }

    pub async fn execution_phase(&self) -> Result<Vec<ShinkaiMessage>, Box<dyn Error>> {
        unimplemented!()
    }
}
