use super::job_prompts::JobPromptGenerator;
use crate::agent::agent::Agent;
use crate::agent::error::AgentError;
use crate::agent::job::{Job, JobLike};
use crate::agent::job_manager::AgentManager;
use crate::db::ShinkaiDB;
use crate::resources::bert_cpp::BertCPPProcess;
use crate::resources::file_parsing::FileParser;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::shinkai_utils::job_scope::LocalScopeEntry;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::JobMessage},
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::source::{SourceDocumentType, SourceFile, SourceFileType};
use std::result::Result::Ok;
use std::time::Instant;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

impl AgentManager {
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

            // Todo: Implement unprocessed messages/queuing logic
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

            // let current_unprocessed_message_count = ...
            shinkai_db.add_to_unprocessed_messages_list(job.job_id().to_string(), job_message.content.clone())?;

            std::mem::drop(shinkai_db); // require to avoid deadlock

            // Fetch data we need to execute job step
            let (mut full_job, agent_found, profile_name, user_profile) =
                self.fetch_relevant_job_data(job.job_id()).await?;

            //
            if !job_message.files_inbox.is_empty() {
                println!(
                    "process_job_message> processing files_map: ... files: {}",
                    job_message.files_inbox.len()
                );
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

                for (_, value) in new_scope_entries {
                    if !full_job.scope.local.contains(&value) {
                        full_job.scope.local.push(value);
                    } else {
                        println!("Duplicate LocalScopeEntry detected");
                    }
                }
                {
                    let mut shinkai_db = self.db.lock().await;
                    shinkai_db.update_job_scope(job.job_id().to_string(), full_job.scope.clone())?;
                    eprintln!(">>> job_scope updated");
                }
            } else {
                // TODO: move this somewhere else
                let mut shinkai_db = self.db.lock().await;
                shinkai_db.init_profile_resource_router(&profile)?;
                std::mem::drop(shinkai_db); // required to avoid deadlock
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
                .process_inference_chain(job_message, full_job, agent_found, profile_name, user_profile)
                .await?;

            // After analysis phase, we execute the resulting execution plan
            //    let executor = PlanExecutor::new(agent, execution_plan)?;
            //    executor.execute_plan();

            return Ok(job_id.clone());
        } else {
            return Err(AgentError::JobNotFound);
        }
    }

    /// Processes the provided message & job data, routes them to a specific inference chain,
    /// and then parses + saves the output result to the DB.
    pub async fn process_inference_chain(
        &self,
        job_message: JobMessage,
        full_job: Job,
        agent_found: Option<Arc<Mutex<Agent>>>,
        profile_name: String,
        user_profile: Option<ShinkaiName>,
    ) -> Result<(), AgentError> {
        let job_id = full_job.job_id().to_string();
        eprintln!("process_inference_chain> full_job: {:?}", full_job);

        // Setup initial data to get ready to call a specific inference chain
        let prev_execution_context = full_job.execution_context.clone();
        let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let generator = RemoteEmbeddingGenerator::new_default();
        let start = Instant::now();

        // Call the inference chain router to choose which chain to use, and call it
        let (inference_response_content, new_execution_context) = self
            .inference_chain_router(
                agent_found,
                full_job,
                job_message.clone(),
                prev_execution_context,
                &generator,
                user_profile,
            )
            .await?;
        let duration = start.elapsed();
        println!("Time elapsed for inference chain processing is: {:?}", duration);

        // Prepare data to save inference response to the DB
        let identity_secret_key_clone = clone_signature_secret_key(&self.identity_secret_key);
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_agent(
            job_id.to_string(),
            inference_response_content.to_string(),
            identity_secret_key_clone,
            profile_name.clone(),
            profile_name.clone(),
        )
        .unwrap();
        // Save response data to DB
        let mut shinkai_db = self.db.lock().await;
        shinkai_db.add_step_history(job_message.job_id.clone(), job_message.content)?;
        shinkai_db.add_step_history(job_message.job_id.clone(), inference_response_content.to_string())?;
        shinkai_db.add_message_to_job_inbox(&job_message.job_id.clone(), &shinkai_message)?;
        shinkai_db.set_job_execution_context(&job_message.job_id.clone(), new_execution_context)?;

        std::mem::drop(bert_process);

        Ok(())
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
