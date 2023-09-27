use super::error::JobManagerError;
use super::IdentityManager;
use crate::agent::agent::Agent;
use crate::agent::job::{Job, JobId, JobLike};
pub use crate::agent::job_execution::*;
use crate::agent::plan_executor::PlanExecutor;
use crate::db::{db_errors::ShinkaiDBError, ShinkaiDB};
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

pub struct JobManager {
    pub agent_manager: Arc<Mutex<AgentManager>>,
    pub job_manager_receiver: Arc<Mutex<mpsc::Receiver<(Vec<JobPreMessage>, JobId)>>>,
    pub job_manager_sender: mpsc::Sender<(Vec<JobPreMessage>, JobId)>,
    pub identity_secret_key: SignatureStaticKey,
    pub node_profile_name: ShinkaiName,
}

impl JobManager {
    pub async fn new(
        db: Arc<Mutex<ShinkaiDB>>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        identity_secret_key: SignatureStaticKey,
        node_profile_name: ShinkaiName,
    ) -> Self {
        let (job_manager_sender, job_manager_receiver) = tokio::sync::mpsc::channel(100);
        let agent_manager = AgentManager::new(db, identity_manager, job_manager_sender.clone()).await;

        let mut job_manager = Self {
            agent_manager: Arc::new(Mutex::new(agent_manager)),
            job_manager_receiver: Arc::new(Mutex::new(job_manager_receiver)),
            job_manager_sender: job_manager_sender.clone(),
            identity_secret_key,
            node_profile_name,
        };
        job_manager.process_received_messages().await;
        job_manager
    }

    pub async fn process_job_message(&mut self, shinkai_message: ShinkaiMessage) -> Result<String, JobManagerError> {
        let mut agent_manager = self.agent_manager.lock().await;
        if agent_manager.is_job_message(shinkai_message.clone()) {
            agent_manager.process_job_message(shinkai_message).await
        } else {
            Err(JobManagerError::NotAJobMessage)
        }
    }

    pub async fn process_received_messages(&mut self) {
        let agent_manager = Arc::clone(&self.agent_manager);
        let receiver = Arc::clone(&self.job_manager_receiver);
        let node_profile_name_clone = self.node_profile_name.clone();
        let identity_secret_key_clone = clone_signature_secret_key(&self.identity_secret_key);
        tokio::spawn(async move {
            while let Some((messages, job_id)) = receiver.lock().await.recv().await {
                for message in messages {
                    let mut agent_manager = agent_manager.lock().await;

                    let shinkai_message_result = ShinkaiMessageBuilder::job_message_from_agent(
                        job_id.clone(),
                        message.content.clone(),
                        clone_signature_secret_key(&identity_secret_key_clone),
                        node_profile_name_clone.to_string(),
                        node_profile_name_clone.to_string(),
                    );

                    if let Ok(shinkai_message) = shinkai_message_result {
                        if let Err(err) = agent_manager
                            .handle_pre_message_schema(message, job_id.clone(), shinkai_message)
                            .await
                        {
                            eprintln!("Error while handling pre message schema: {:?}", err);
                        }
                    } else if let Err(err) = shinkai_message_result {
                        eprintln!("Error while building ShinkaiMessage: {:?}", err);
                    }
                }
            }
        });
    }
}

pub struct AgentManager {
    pub jobs: Arc<Mutex<HashMap<String, Box<dyn JobLike>>>>,
    pub db: Arc<Mutex<ShinkaiDB>>,
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    pub job_manager_sender: mpsc::Sender<(Vec<JobPreMessage>, JobId)>,
    pub agents: Vec<Arc<Mutex<Agent>>>,
}

impl AgentManager {
    pub async fn new(
        db: Arc<Mutex<ShinkaiDB>>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager_sender: mpsc::Sender<(Vec<JobPreMessage>, JobId)>,
    ) -> Self {
        let jobs_map = Arc::new(Mutex::new(HashMap::new()));
        {
            let shinkai_db = db.lock().await;
            let all_jobs = shinkai_db.get_all_jobs().unwrap();
            let mut jobs = jobs_map.lock().await;
            for job in all_jobs {
                jobs.insert(job.job_id().to_string(), job);
            }
        }

        // Get all serialized_agents and convert them to Agents
        let mut agents: Vec<Arc<Mutex<Agent>>> = Vec::new();
        {
            let identity_manager = identity_manager.lock().await;
            let serialized_agents = identity_manager.get_all_agents().await.unwrap();
            for serialized_agent in serialized_agents {
                let agent = Agent::from_serialized_agent(serialized_agent, job_manager_sender.clone());
                agents.push(Arc::new(Mutex::new(agent)));
            }
        }

        let mut job_manager = Self {
            jobs: jobs_map,
            db,
            job_manager_sender: job_manager_sender.clone(),
            identity_manager,
            agents,
        };

        job_manager
    }

    /// Checks that the provided ShinkaiMessage is an unencrypted job message
    pub fn is_job_message(&mut self, message: ShinkaiMessage) -> bool {
        match &message.body {
            MessageBody::Unencrypted(body) => match &body.message_data {
                MessageData::Unencrypted(data) => match data.message_content_schema {
                    MessageSchemaType::JobCreationSchema | MessageSchemaType::JobMessageSchema => true,
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        }
    }

    /// Processes a job creation message
    pub async fn process_job_creation(
        &mut self,
        job_creation: JobCreationInfo,
        agent_id: &String,
    ) -> Result<String, JobManagerError> {
        let job_id = format!("jobid_{}", uuid::Uuid::new_v4());
        {
            let mut shinkai_db = self.db.lock().await;
            match shinkai_db.create_new_job(job_id.clone(), agent_id.clone(), job_creation.scope) {
                Ok(_) => (),
                Err(err) => return Err(JobManagerError::ShinkaiDB(err)),
            };

            match shinkai_db.get_job(&job_id) {
                Ok(job) => {
                    std::mem::drop(shinkai_db); // require to avoid deadlock
                    self.jobs.lock().await.insert(job_id.clone(), Box::new(job));
                    let mut agent_found = None;
                    for agent in &self.agents {
                        let locked_agent = agent.lock().await;
                        if &locked_agent.id == agent_id {
                            agent_found = Some(agent.clone());
                            break;
                        }
                    }

                    if agent_found.is_none() {
                        let identity_manager = self.identity_manager.lock().await;
                        if let Some(serialized_agent) = identity_manager.search_local_agent(&agent_id).await {
                            let agent = Agent::from_serialized_agent(serialized_agent, self.job_manager_sender.clone());
                            agent_found = Some(Arc::new(Mutex::new(agent)));
                            self.agents.push(agent_found.clone().unwrap());
                        }
                    }

                    let job_id_to_return = match agent_found {
                        Some(_) => Ok(job_id.clone()),
                        None => Err(anyhow::Error::new(JobManagerError::AgentNotFound)),
                    };

                    job_id_to_return.map_err(|_| JobManagerError::AgentNotFound)
                }
                Err(err) => {
                    return Err(JobManagerError::ShinkaiDB(err));
                }
            }
        }
    }

    /// Adds pre-message to job inbox
    pub async fn handle_pre_message_schema(
        &mut self,
        pre_message: JobPreMessage,
        job_id: String,
        shinkai_message: ShinkaiMessage,
    ) -> Result<String, JobManagerError> {
        println!("handle_pre_message_schema> pre_message: {:?}", pre_message);

        self.db
            .lock()
            .await
            .add_message_to_job_inbox(job_id.as_str(), &shinkai_message)?;
        Ok(String::new())
    }

    pub async fn process_job_message(&mut self, message: ShinkaiMessage) -> Result<String, JobManagerError> {
        match message.clone().body {
            MessageBody::Unencrypted(body) => {
                match body.message_data {
                    MessageData::Unencrypted(data) => {
                        let message_type = data.message_content_schema;
                        match message_type {
                            MessageSchemaType::JobCreationSchema => {
                                let agent_name =
                                    ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                                let agent_id = agent_name.get_agent_name().ok_or(JobManagerError::AgentNotFound)?;
                                let job_creation: JobCreationInfo = serde_json::from_str(&data.message_raw_content)
                                    .map_err(|_| JobManagerError::ContentParseFailed)?;
                                self.process_job_creation(job_creation, &agent_id).await
                            }
                            MessageSchemaType::JobMessageSchema => {
                                let job_message: JobMessage = serde_json::from_str(&data.message_raw_content)
                                    .map_err(|_| JobManagerError::ContentParseFailed)?;
                                self.process_job_step(message, job_message).await
                            }
                            _ => {
                                // Handle Empty message type if needed, or return an error if it's not a valid job message
                                Err(JobManagerError::NotAJobMessage)
                            }
                        }
                    }
                    _ => Err(JobManagerError::NotAJobMessage),
                }
            }
            _ => Err(JobManagerError::NotAJobMessage),
        }
    }
}
