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
use chrono::Utc;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use serde_json::{Map, Value as JsonValue};
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
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
use shinkai_vector_resources::source::VRSource;
use std::fmt;
use std::result::Result::Ok;
use std::{collections::HashMap, error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl AgentManager {
    /// Processes a job message which will trigger a job step
    pub async fn process_job_step(
        &mut self,
        message: ShinkaiMessage,
        job_message: JobMessage,
    ) -> Result<String, AgentError> {
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

            // TODO(Nico): adding embeddings here to test. needs to be moved out
            // - Go over the files
            // - Check if they are parseable (for now just pdfs)
            // - if they are parseable, then parse them and add them to the db

            if !job_message.files_inbox.is_empty() {
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
                println!(
                    "process_job_message> processing files_map: ... files: {}",
                    job_message.files_inbox.len()
                );
                // TODO: later we  should able to grab errors and return them to the user
                AgentManager::process_message_multifiles(self.db.clone(), job_message.files_inbox.clone(), profile)
                    .await?;
            }

            // TODO(Nico): Notes from conversation with Rob
            // create a job
            // check box whether to save added documents to db permanantly
            // user sends messages with files, files get vector resources generated automatically (if can be ingested)
            // If not saving to db permanantly, then the vec resource is serialized and saved into the local job scope
            // If are saving to db permanantly, then the vec resource is saved to db directly, and pointer is added to remote job scope
            // User closes job after finishing, if not saving by default, ask user whether they want to save the document to the DB

            // TODO(Nico): move this to a parallel thread that runs in the background
            let _ = self.analysis_phase(&**job, job_message.clone()).await?;

            // After analysis phase, we execute the resulting execution plan
            //    let executor = PlanExecutor::new(agent, execution_plan)?;
            //    executor.execute_plan();

            return Ok(job_message.job_id.clone());
        } else {
            return Err(AgentError::JobNotFound);
        }
    }

    // Begins processing the analysis phase of the job
    pub async fn analysis_phase(&self, job: &dyn JobLike, job_message: JobMessage) -> Result<(), AgentError> {
        // Fetch the job
        let job_id = job.job_id().to_string();
        let full_job = { self.db.lock().await.get_job(&job_id)? };

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

        // Setup initial data to start moving through analysis phase
        let prev_execution_context = full_job.execution_context.clone();
        let analysis_context = HashMap::new();

        // Embedding search
        // TODO: add start of time measurement here

        let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let generator = RemoteEmbeddingGenerator::new_default();
        let query = generator.generate_embedding_default(job_message.content.clone().as_str()).unwrap();
        let shinkai_db = self.db.lock().await;
        let ret_data_chunks = shinkai_db
            .vector_search_tolerance_ranged(query, 10, 0.4, &user_profile.unwrap())
            .unwrap();
        eprintln!(
            "ret_data_chunks: {:?}",
            ret_data_chunks.get(0).unwrap().chunk.get_data_string()
        );
        let embedding_chunks: Vec<String> = ret_data_chunks
            .iter()
            .filter_map(|data_chunk| data_chunk.chunk.get_data_string().ok())
            .collect();
        std::mem::drop(shinkai_db);
        std::mem::drop(bert_process);

        // TODO: end time measurement here and print to console

        // TODO: Later implement all analysis phase chaining/branching logic starting from here
        // and have multiple methods like process_analysis_inference which use different
        // prompts and are called as needed to arrive at a full execution plan ready to be returned

        let inference_response = match agent_found {
            Some(agent) => {
                self.process_analysis_inference(
                    full_job,
                    job_message.content.clone(),
                    agent,
                    embedding_chunks,
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

        // there is no deadlock here because it goes out of scope right after
        // std::mem::drop(shinkai_db); // require to avoid deadlock

        Ok(())
    }

    /// Temporary method that does no chaining/advanced prompting/context usage,
    /// but simply inferences the LLM to get a direct response back
    async fn process_analysis_inference(
        &self,
        job: Job,
        message: String,
        agent: Arc<Mutex<Agent>>,
        embeddings: Vec<String>,
        execution_context: HashMap<String, String>,
        analysis_context: HashMap<String, String>,
    ) -> Result<JsonValue, Box<dyn Error>> {
        println!("analysis_inference>  message: {:?}", message);

        // Generate the needed prompt
        let mut filled_prompt = JobPromptGenerator::basic_instant_response_prompt(message.clone());
        filled_prompt.add_vector_chunk_response(embeddings);

        // Execute LLM inferencing
        let agent_cloned = agent.clone();
        let response = tokio::spawn(async move {
            let mut agent = agent_cloned.lock().await;
            agent.inference(filled_prompt).await
        })
        .await?;

        println!("analysis_inference> response: {:?}", response);

        // TODO: Later update all methods to AgentError
        Ok(response.unwrap())

        // TODO: Later implement re-run logic like below, but as a while loop in analysis phase/some wrapper retry method.
        // Because we can't do normal recursion in async.
        //
        // match response {
        //     Err(AgentError::FailedExtractingJSONObjectFromResponse(s)) => {
        //         println!("{}", s);
        //         let new_message = format!("{}. You must return a valid JSON object as a response.", message);
        //         self.process_analysis_inference(job, new_message, agent, execution_context, analysis_context)
        //             .await
        //     }
        //     _ => Ok(response.unwrap()),
        // }
    }

    pub async fn execution_phase(&self) -> Result<Vec<ShinkaiMessage>, Box<dyn Error>> {
        unimplemented!()
    }

    // TODO(Nico): refactor so it's decomposed
    pub async fn process_message_multifiles(
        db: Arc<Mutex<ShinkaiDB>>,
        files_inbox: String,
        profile: ShinkaiName,
    ) -> Result<HashMap<String, Vec<u8>>, AgentError> {
        let _bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let mut shinkai_db = db.lock().await;
        let files_result = shinkai_db.get_all_files_from_inbox(files_inbox.clone());

        // Check if there was an error getting the files
        let files = match files_result {
            Ok(files) => files,
            Err(e) => return Err(AgentError::ShinkaiDB(e)),
        };

        // Convert the Vec<(String, Vec<u8>)> to a HashMap<String, Vec<u8>>
        let mut files_map: HashMap<String, Vec<u8>> = HashMap::new();

        // Create the RemoteEmbeddingGenerator instance
        let generator = Arc::new(RemoteEmbeddingGenerator::new_default());

        for (filename, content) in files.into_iter() {
            if filename.ends_with(".pdf") {
                eprintln!("Processing PDF file: {}", filename);
                // Generate embeddings for PDF files
                let desc = "An initial introduction to the Shinkai Network."; // TODO: get the description from the LLM
                let doc = FileParser::parse_pdf(
                    &content,
                    100,
                    &*generator,
                    &filename,
                    Some(desc),
                    VRSource::None, // TODO: how can we get the source? if name starts with http then it's an URL if starts with file then it's local
                    &vec![],        // TODO: should this be computed using RAKE?
                )?;

                let resource = BaseVectorResource::from(doc.clone());
                // eprintln!("resource: {:?}", resource);
                eprintln!("profile: {:?}", profile);
                shinkai_db.init_profile_resource_router(&profile)?;
                shinkai_db.save_resource(resource, &profile).unwrap();
            }
            files_map.insert(filename, content);
        }

        Ok(files_map)
    }
}
