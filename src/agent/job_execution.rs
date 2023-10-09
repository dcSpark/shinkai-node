use crate::agent::agent::Agent;
use crate::agent::error::AgentError;
use crate::agent::job::{Job, JobId, JobLike};
use crate::agent::job_manager::{AgentManager, JobManager};
use crate::agent::job_prompts::JobPromptGenerator;
use crate::agent::plan_executor::PlanExecutor;
use crate::db::{db_errors::ShinkaiDBError, ShinkaiDB};
use crate::resources::bert_cpp::BertCPPProcess;
use crate::resources::file_parsing::FileParser;
use crate::schemas::identity::Identity;
use blake3::Hasher;
use chrono::Utc;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use serde_json::{Map, Value as JsonValue};
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::job_scope::LocalScopeEntry;
use shinkai_message_primitives::{
    schemas::shinkai_name::{ShinkaiName, ShinkaiNameError},
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{JobCreationInfo, JobMessage, JobPreMessage, MessageSchemaType},
    },
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::data_tags::DataTag;
use shinkai_vector_resources::document_resource::DocumentVectorResource;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::resource_errors::VectorResourceError;
use shinkai_vector_resources::source::{SourceDocumentType, SourceFile, SourceFileType, VRSource};
use std::fmt;
use std::result::Result::Ok;
use std::time::Instant;
use std::{collections::HashMap, error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl AgentManager {
    /// Fetches boilerplate/relevant data required for a job to process a step
    async fn fetch_relevant_job_data(
        &self,
        job_id: &str,
    ) -> Result<(Job, Option<Arc<Mutex<Agent>>>, String, Option<ShinkaiName>), AgentError> {
        // Fetch the job
        let full_job = { self.db.lock().await.get_job(job_id)? };

        // Acquire Agent
        let agent_id = full_job.parent_agent_id.clone();
        let mut agent_found = None;
        let mut profile_name = String::new();
        let mut user_profile: Option<ShinkaiName> = None;
        for agent in &self.agents {
            let locked_agent = agent.lock().await;
            if locked_agent.id == agent_id {
                agent_found = Some(agent.clone());
                profile_name = locked_agent.full_identity_name.full_name.clone();
                user_profile = Some(locked_agent.full_identity_name.extract_profile().unwrap());
                break;
            }
        }

        Ok((full_job, agent_found, profile_name, user_profile))
    }

    /// Processes a job message which will trigger a job step
    pub async fn process_job_step(
        &mut self,
        message: ShinkaiMessage,
        job_message: JobMessage,
    ) -> Result<String, AgentError> {
        if let Some(job) = self.jobs.lock().await.get(&job_message.job_id) {
            // Basic setup
            let job = job.clone();
            let job_id = job.job_id().to_string();
            let mut shinkai_db = self.db.lock().await;
            shinkai_db.add_message_to_job_inbox(&job_message.job_id.clone(), &message)?;
            println!("process_job_step> job_message: {:?}", job_message);

            // Verify identity/profile match
            let sender_subidentity_result =
                ShinkaiName::from_shinkai_message_using_sender_subidentity(&message.clone());
            let sender_subidentity = match sender_subidentity_result {
                Ok(subidentity) => subidentity,
                Err(e) => return Err(AgentError::InvalidSubidentity(e)),
            };
            let profile_result = sender_subidentity.extract_profile();
            let profile = match profile_result {
                Ok(profile) => profile,
                Err(e) => return Err(AgentError::InvalidProfileSubidentity(e.to_string())),
            };

            // Fetch data we need to execute job step
            let (full_job, agent_found, profile_name, user_profile) =
                self.fetch_relevant_job_data(job.job_id()).await?;

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

            // TODO(Nico): adding embeddings here to test. needs to be moved out
            // - Go over the files
            // - Check if they are parseable (for now just pdfs)
            // - if they are parseable, then parse them and add them to the db
            if !job_message.files_inbox.is_empty() {
                println!(
                    "process_job_message> processing files_map: ... files: {}",
                    job_message.files_inbox.len()
                );
                {
                    // TODO: later we should able to grab errors and return them to the user
                    let new_scope_entries = match agent_found.clone() {
                        Some(agent) => {
                            let resp = AgentManager::process_message_multifiles(
                                self.db.clone(),
                                agent,
                                job_message.files_inbox.clone(),
                                profile,
                            )
                            .await?;
                            resp
                        }
                        None => {
                            // Handle the None case here. For example, you might want to return an error:
                            return Err(AgentError::AgentNotFound);
                        }
                    };

                    eprintln!(">>> new_scope_entries: {:?}", new_scope_entries.keys());

                    let mut job_scope = full_job.scope.clone();
                    for (_, value) in new_scope_entries {
                        if !job_scope.local.contains(&value) {
                            job_scope.local.push(value);
                        } else {
                            println!("Duplicate LocalScopeEntry detected");
                        }
                    }
                    {
                        let mut shinkai_db = self.db.lock().await;
                        shinkai_db.update_job_scope(job.job_id().to_string(), job_scope)?;
                        eprintln!(">>> job_scope updated");
                    }
                }
            } else {
                // TODO: move this somewhere else
                let mut shinkai_db = self.db.lock().await;
                shinkai_db.init_profile_resource_router(&profile)?;
                std::mem::drop(shinkai_db); // require to avoid deadlock
            }

            // TODO(Nico): Notes from conversation with Rob
            // create a job
            // check box whether to save added documents to db permanantly
            // user sends messages with files, files get vector resources generated automatically (if can be ingested)
            // If not saving to db permanantly, then the vec resource is serialized and saved into the local job scope
            // If are saving to db permanantly, then the vec resource is saved to db directly, and pointer is added to remote job scope
            // User closes job after finishing, if not saving by default, ask user whether they want to save the document to the DB

            // TODO(Nico): move this to a parallel thread that runs in the background
            let _ = self
                .analysis_phase(&**job, job_message, full_job, agent_found, profile_name, user_profile)
                .await?;

            // After analysis phase, we execute the resulting execution plan
            //    let executor = PlanExecutor::new(agent, execution_plan)?;
            //    executor.execute_plan();

            return Ok(job_id.clone());
        } else {
            return Err(AgentError::JobNotFound);
        }
    }

    // Begins processing the analysis phase of the job
    pub async fn analysis_phase(
        &self,
        job: &dyn JobLike,
        job_message: JobMessage,
        full_job: Job,
        agent_found: Option<Arc<Mutex<Agent>>>,
        profile_name: String,
        user_profile: Option<ShinkaiName>,
    ) -> Result<(), AgentError> {
        // Fetch the job
        let job_id = job.job_id().to_string();
        eprintln!("analysis_phase> full_job: {:?}", full_job);

        // Setup initial data to start moving through analysis phase
        let prev_execution_context = full_job.execution_context.clone();
        let analysis_context = HashMap::new();

        let start = Instant::now();
        let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let generator = RemoteEmbeddingGenerator::new_default();
        let query = generator
            .generate_embedding_default(job_message.content.clone().as_str())
            .unwrap();
        let shinkai_db = self.db.lock().await;

        // Perform a vector search to find
        let ret_data_chunks = shinkai_db
            .vector_search_tolerance_ranged(query, 2, 0.4, &user_profile.unwrap())
            .unwrap();

        let mut data_chunks_content: Vec<String> = Vec::new();
        if !ret_data_chunks.is_empty() {
            data_chunks_content = ret_data_chunks
                .iter()
                .filter_map(|data_chunk| data_chunk.chunk.get_data_string().ok())
                .collect();
        }

        let duration = start.elapsed();
        eprintln!("Time elapsed in parsing the embeddings is: {:?}", duration);
        std::mem::drop(shinkai_db);
        std::mem::drop(bert_process);

        // TODO: Later implement all analysis phase chaining/branching logic starting from here
        // and have multiple methods like process_analysis_inference which use different
        // prompts and are called as needed to arrive at a full execution plan ready to be returned

        let inference_response = match agent_found {
            Some(agent) => {
                self.process_analysis_inference(
                    full_job,
                    job_message.content.clone(),
                    agent,
                    data_chunks_content,
                    prev_execution_context,
                    analysis_context,
                )
                .await
            }
            None => Err(Box::new(AgentError::AgentNotFound) as Box<dyn std::error::Error>),
        }?;
        let inference_content = match inference_response.get("answer") {
            Some(answer) => answer
                .as_str()
                .ok_or_else(|| AgentError::InferenceJSONResponseMissingField("answer".to_string()))?,
            None => Err(AgentError::InferenceJSONResponseMissingField("answer".to_string()))?,
        };

        // Save inference response to job inbox
        let identity_secret_key_clone = clone_signature_secret_key(&self.identity_secret_key);
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_agent(
            job_id.clone(),
            inference_content.to_string(),
            identity_secret_key_clone,
            profile_name.clone(),
            profile_name.clone(),
        )
        .unwrap();

        // Save the step history
        let mut shinkai_db = self.db.lock().await;
        shinkai_db.add_step_history(job_message.job_id.clone(), job_message.content)?;
        shinkai_db.add_step_history(job_message.job_id.clone(), inference_response.to_string())?;
        shinkai_db.add_message_to_job_inbox(&job_message.job_id.clone(), &shinkai_message)?;

        Ok(())
    }

    /// Temporary method that does no chaining/advanced prompting/context usage,
    /// but simply inferences the LLM to get a direct response back
    async fn process_analysis_inference(
        &self,
        job: Job,
        message: String,
        agent: Arc<Mutex<Agent>>,
        data_chunks_content: Vec<String>,
        execution_context: HashMap<String, String>,
        analysis_context: HashMap<String, String>,
    ) -> Result<JsonValue, Box<dyn Error>> {
        println!("analysis_inference>  message: {:?}", message);

        // Generate the needed prompt
        let filled_prompt =
            JobPromptGenerator::response_prompt_with_vector_search(message.clone(), data_chunks_content);

        // Execute LLM inferencing
        let agent_cloned = agent.clone();
        let response = tokio::spawn(async move {
            let mut agent = agent_cloned.lock().await;
            agent.inference(filled_prompt).await
        })
        .await?;

        println!("analysis_inference> response: {:?}", response);

        match response {
            Ok(json) => Ok(json),
            Err(AgentError::FailedExtractingJSONObjectFromResponse(text)) => {
                eprintln!("Retrying inference with new prompt");
                self.json_not_found_retry(agent.clone(), text.clone()).await
            }
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn json_not_found_retry(&self, agent: Arc<Mutex<Agent>>, text: String) -> Result<JsonValue, Box<dyn Error>> {
        let response = tokio::spawn(async move {
            let mut agent = agent.lock().await;
            let prompt = JobPromptGenerator::basic_json_retry_response_prompt(text);
            agent.inference(prompt).await
        })
        .await?;
        Ok(response?)
    }

    pub async fn execution_phase(&self) -> Result<Vec<ShinkaiMessage>, Box<dyn Error>> {
        unimplemented!()
    }

    fn create_vrsource(filename: &str, file_type: SourceFileType, content_hash: Option<String>) -> VRSource {
        if filename.starts_with("http") {
            let filename_without_extension = filename.trim_end_matches(".pdf");
            VRSource::new_uri_ref(filename_without_extension)
        } else if filename.starts_with("file") {
            let filename_without_extension = filename.trim_start_matches("file://").trim_end_matches(".pdf");
            VRSource::new_source_file_ref(filename_without_extension.to_string(), file_type, content_hash.unwrap())
        } else {
            VRSource::none()
        }
    }

    // TODO(Nico): refactor so it's decomposed
    pub async fn process_message_multifiles(
        db: Arc<Mutex<ShinkaiDB>>,
        agent: Arc<Mutex<Agent>>,
        files_inbox: String,
        profile: ShinkaiName,
    ) -> Result<HashMap<String, LocalScopeEntry>, AgentError> {
        let _bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let mut shinkai_db = db.lock().await;
        let files_result = shinkai_db.get_all_files_from_inbox(files_inbox.clone());

        // Check if there was an error getting the files
        let files = match files_result {
            Ok(files) => files,
            Err(e) => return Err(AgentError::ShinkaiDB(e)),
        };

        let mut files_map: HashMap<String, LocalScopeEntry> = HashMap::new();

        // Create the RemoteEmbeddingGenerator instance
        let generator = Arc::new(RemoteEmbeddingGenerator::new_default());

        for (filename, content) in files.into_iter() {
            eprintln!("Iterating over file: {}", filename);
            if filename.ends_with(".pdf") {
                eprintln!("Processing PDF file: {}", filename);
                let pdf_overview = FileParser::parse_pdf_for_keywords_and_description(&content, 3, 200)?;

                let agent_clone = agent.clone();
                let grouped_text_list_clone = pdf_overview.grouped_text_list.clone();
                let description_response = tokio::spawn(async move {
                    let mut agent = agent_clone.lock().await;
                    let prompt = JobPromptGenerator::simple_doc_description(grouped_text_list_clone);
                    agent.inference(prompt).await
                })
                .await?;

                // TODO: Maybe add: "\nKeywords: keywords_generated_by_RAKE"?
                eprintln!("description_response: {:?}", description_response);

                let vrsource = Self::create_vrsource(
                    &filename,
                    SourceFileType::Document(SourceDocumentType::Pdf),
                    Some(pdf_overview.blake3_hash),
                );
                eprintln!("vrsource: {:?}", vrsource);
                let doc = FileParser::parse_pdf(
                    &content,
                    150,
                    &*generator,
                    &filename,
                    Some(&"".to_string()),
                    vrsource,
                    &vec![],
                )?;

                let resource = BaseVectorResource::from(doc.clone());
                // eprintln!("resource: {:?}", resource);
                eprintln!("profile: {:?}", profile);
                shinkai_db.init_profile_resource_router(&profile)?;
                shinkai_db.save_resource(resource, &profile).unwrap();

                let local_scope_entry = LocalScopeEntry {
                    resource: BaseVectorResource::from(doc.clone()),
                    source: SourceFile::new(
                        filename.clone(),
                        SourceFileType::Document(SourceDocumentType::Pdf),
                        content,
                    ),
                };
                files_map.insert(filename, local_scope_entry);
            }
        }

        Ok(files_map)
    }
}
