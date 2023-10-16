use super::job_prompts::JobPromptGenerator;
use crate::agent::agent::Agent;
use crate::agent::error::AgentError;
use crate::agent::file_parsing::ParsingHelper;
use crate::agent::job::{Job, JobLike};
use crate::agent::job_manager::JobManager;
use crate::agent::queue::job_queue_manager::JobForProcessing;
use crate::db::ShinkaiDB;
use crate::resources::bert_cpp::BertCPPProcess;
use ed25519_dalek::SecretKey as SignatureStaticKey;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::shinkai_utils::job_scope::{DBScopeEntry, LocalScopeEntry, ScopeEntry};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::JobMessage},
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::source::{SourceDocumentType, SourceFile, SourceFileType, VRSource};
use shinkai_vector_resources::vector_resource::VectorResource;
use std::result::Result::Ok;
use std::time::Instant;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

impl JobManager {
    /// Processes a job message which will trigger a job step
    pub async fn process_job_message_queued(
        job_message: JobForProcessing,
        db: Arc<Mutex<ShinkaiDB>>,
        identity_secret_key: SignatureStaticKey,
    ) -> Result<String, AgentError> {
        let job_id = job_message.job_message.job_id.clone();
        // Fetch data we need to execute job step
        let (mut full_job, agent_found, profile_name, user_profile) =
            JobManager::fetch_relevant_job_data(&job_message.job_message.job_id, db.clone()).await?;

        let _bert_process = BertCPPProcess::start(); // Gets killed if out of scope

        // Processes any files which were sent with the job message
        JobManager::process_job_message_files(
            db.clone(),
            &job_message.job_message,
            agent_found.clone(),
            &mut full_job,
            job_message.profile,
            false,
        )
        .await?;

        let _ = JobManager::process_inference_chain(
            db,
            identity_secret_key,
            job_message.job_message,
            full_job,
            agent_found.clone(),
            profile_name,
            user_profile,
        )
        .await?;

        return Ok(job_id.clone());
    }

    /// Processes the provided message & job data, routes them to a specific inference chain,
    /// and then parses + saves the output result to the DB.
    pub async fn process_inference_chain(
        db: Arc<Mutex<ShinkaiDB>>,
        identity_secret_key: SignatureStaticKey,
        job_message: JobMessage,
        full_job: Job,
        agent_found: Option<SerializedAgent>,
        profile_name: String,
        user_profile: Option<ShinkaiName>,
    ) -> Result<(), AgentError> {
        let job_id = full_job.job_id().to_string();
        eprintln!("process_inference_chain> full_job: {:?}", full_job);

        // Setup initial data to get ready to call a specific inference chain
        let prev_execution_context = full_job.execution_context.clone();
        // let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let generator = RemoteEmbeddingGenerator::new_default();
        let start = Instant::now();

        // Call the inference chain router to choose which chain to use, and call it
        let (inference_response_content, new_execution_context) = JobManager::inference_chain_router(
            db.clone(),
            agent_found,
            full_job,
            job_message.clone(),
            prev_execution_context,
            &generator,
            user_profile,
        )
        .await?;
        let duration = start.elapsed();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Time elapsed for inference chain processing is: {:?}", duration),
        );

        // Prepare data to save inference response to the DB
        let identity_secret_key_clone = clone_signature_secret_key(&identity_secret_key);
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_agent(
            job_id.to_string(),
            inference_response_content.to_string(),
            identity_secret_key_clone,
            profile_name.clone(),
            profile_name.clone(),
        )
        .unwrap();

        // Save response data to DB
        let mut shinkai_db = db.lock().await;
        shinkai_db.add_step_history(job_message.job_id.clone(), job_message.content)?;
        shinkai_db.add_step_history(job_message.job_id.clone(), inference_response_content.to_string())?;
        shinkai_db.add_message_to_job_inbox(&job_message.job_id.clone(), &shinkai_message)?;
        shinkai_db.set_job_execution_context(&job_message.job_id.clone(), new_execution_context)?;

        Ok(())
    }

    /// Processes the files sent together with the current job_message into Vector Resources,
    /// and saves them either into the local job scope, or the DB depending on `save_to_db_directly`.
    pub async fn process_job_message_files(
        db: Arc<Mutex<ShinkaiDB>>,
        job_message: &JobMessage,
        agent_found: Option<SerializedAgent>,
        full_job: &mut Job,
        profile: ShinkaiName,
        save_to_db_directly: bool,
    ) -> Result<(), AgentError> {
        if !job_message.files_inbox.is_empty() {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Processing files_map: ... files: {}", job_message.files_inbox.len()).as_str(),
            );
            // TODO: later we should able to grab errors and return them to the user
            let new_scope_entries = JobManager::process_files_inbox(
                db.clone(),
                agent_found,
                job_message.files_inbox.clone(),
                profile,
                save_to_db_directly,
            )
            .await?;

            for (_, value) in new_scope_entries {
                match value {
                    ScopeEntry::Local(local_entry) => {
                        if !full_job.scope.local.contains(&local_entry) {
                            full_job.scope.local.push(local_entry);
                        } else {
                            println!("Duplicate LocalScopeEntry detected");
                        }
                    }
                    ScopeEntry::Database(db_entry) => {
                        if !full_job.scope.database.contains(&db_entry) {
                            full_job.scope.database.push(db_entry);
                        } else {
                            println!("Duplicate DBScopeEntry detected");
                        }
                    }
                }
            }
            {
                let mut shinkai_db = db.lock().await;
                shinkai_db.update_job_scope(full_job.job_id().to_string(), full_job.scope.clone())?;
            }
        } else {
            // TODO: move this somewhere else
            let mut shinkai_db = db.lock().await;
            shinkai_db.init_profile_resource_router(&profile)?;
            std::mem::drop(shinkai_db); // required to avoid deadlock
        }

        Ok(())
    }

    /// Processes the files in a given file inbox by generating VectorResources + job `ScopeEntry`s.
    /// If save_to_db_directly == true, the files will save to the DB and be returned as `DBScopeEntry`s.
    /// Else, the files will be returned as `LocalScopeEntry`s and thus held inside.
    pub async fn process_files_inbox(
        db: Arc<Mutex<ShinkaiDB>>,
        agent: Option<SerializedAgent>,
        files_inbox: String,
        profile: ShinkaiName,
        save_to_db_directly: bool,
    ) -> Result<HashMap<String, ScopeEntry>, AgentError> {
        // Handle the None case if the agent is not found
        let agent = match agent {
            Some(agent) => agent,
            None => return Err(AgentError::AgentNotFound),
        };

        // let _bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let mut shinkai_db = db.lock().await;
        let files_result = shinkai_db.get_all_files_from_inbox(files_inbox.clone());
        // Check if there was an error getting the files
        let files = match files_result {
            Ok(files) => files,
            Err(e) => return Err(AgentError::ShinkaiDB(e)),
        };
        // Create the RemoteEmbeddingGenerator instance
        let generator = Arc::new(RemoteEmbeddingGenerator::new_default());
        let mut files_map: HashMap<String, ScopeEntry> = HashMap::new();

        // Start processing the files
        for (filename, content) in files.into_iter() {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                &format!("Processing file: {}", filename),
            );
            let resource = JobManager::parse_file_into_resource(
                db.clone(),
                content.clone(),
                &*generator,
                filename.clone(),
                None,
                &vec![],
                agent.clone(),
                400,
            )
            .await?;

            // Now create Local/DBScopeEntry depending on setting
            if save_to_db_directly {
                let pointer = resource.as_trait_object().get_resource_pointer();
                shinkai_db.init_profile_resource_router(&profile)?;
                shinkai_db.save_resource(resource, &profile).unwrap();

                let db_scope_entry = DBScopeEntry {
                    resource_pointer: pointer,
                    source: VRSource::from_file(&filename, &content)?,
                };
                files_map.insert(filename, ScopeEntry::Database(db_scope_entry));
            } else {
                let local_scope_entry = LocalScopeEntry {
                    resource: resource,
                    source: SourceFile::new(
                        filename.clone(),
                        SourceFileType::Document(SourceDocumentType::Pdf),
                        content,
                    ),
                };
                files_map.insert(filename, ScopeEntry::Local(local_scope_entry));
            }
        }

        Ok(files_map)
    }
}
