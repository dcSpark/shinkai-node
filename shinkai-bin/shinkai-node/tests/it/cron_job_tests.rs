#[cfg(test)]
mod tests {
    use core::panic;
    use ed25519_dalek::SigningKey;
    use futures::Future;
    use shinkai_message_primitives::{
        schemas::{
            llm_providers::serialized_llm_provider::{LLMProviderInterface, OpenAI, SerializedLLMProvider},
            shinkai_name::ShinkaiName,
        },
        shinkai_utils::{
            encryption::unsafe_deterministic_encryption_keypair,
            shinkai_logging::init_default_tracing,
            signatures::{clone_signature_secret_key, unsafe_deterministic_signature_keypair},
        },
    };
    use shinkai_node::network::ws_manager::WSUpdateHandler;
    use shinkai_node::{
        cron_tasks::cron_manager::{CronManager, CronManagerError},
        db::{db_cron_task::CronTask, ShinkaiDB},
        llm_provider::job_manager::JobManager,
        managers::IdentityManager,
        vector_fs::vector_fs::VectorFS,
    };
    use shinkai_vector_resources::{
        embedding_generator::RemoteEmbeddingGenerator, file_parser::unstructured_api::UnstructuredAPI,
    };
    use std::{env, sync::Weak};
    use std::{fs, path::Path, pin::Pin, sync::Arc, time::Duration};
    use tokio::sync::Mutex;

    const CRON_INTERVAL_TIME: u64 = 60 * 10; // it doesn't matter here

    #[test]
    fn setup() {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn test_process_cron_job() {
        init_default_tracing();
        setup();
        let db = Arc::new(ShinkaiDB::new("db_tests/").unwrap());
        let db_weak = Arc::downgrade(&db);
        let (identity_secret_key, identity_public_key) = unsafe_deterministic_signature_keypair(0);
        let (_, encryption_public_key) = unsafe_deterministic_encryption_keypair(0);
        let node_profile_name = ShinkaiName::new("@@localhost.shinkai/main".to_string()).unwrap();
        let agent_id = "agent_id1".to_string();
        let agent_name =
            ShinkaiName::new(format!("{}/agent/{}", node_profile_name.clone(), agent_id.clone()).to_string()).unwrap();

        {
            // add keys
            match db.update_local_node_keys(node_profile_name.clone(), encryption_public_key, identity_public_key) {
                Ok(_) => (),
                Err(e) => panic!("Failed to update local node keys: {}", e),
            }
        }

        let subidentity_manager = IdentityManager::new(db_weak.clone(), node_profile_name.clone())
            .await
            .unwrap();
        let identity_manager = Arc::new(Mutex::new(subidentity_manager));

        {
            let open_ai = OpenAI {
                model_type: "gpt-3.5-turbo-1106".to_string(),
            };

            let agent = SerializedLLMProvider {
                id: agent_id.clone(),
                full_identity_name: agent_name.clone(),
                perform_locally: false,
                external_url: Some("https://api.openai.com".to_string()),
                api_key: env::var("INITIAL_AGENT_API_KEY").ok(),
                model: LLMProviderInterface::OpenAI(open_ai),
                toolkit_permissions: vec![],
                storage_bucket_permissions: vec![],
                allowed_message_senders: vec![],
            };

            let profile = agent_name.clone().extract_profile().unwrap();

            // add agent
            match db.add_llm_provider(agent.clone(), &profile) {
                Ok(()) => {
                    let mut subidentity_manager = identity_manager.lock().await;
                    match subidentity_manager.add_llm_provider_subidentity(agent).await {
                        Ok(_) => (),
                        Err(err) => {
                            panic!("Failed to add agent subidentity: {}", err);
                        }
                    }
                }
                Err(e) => {
                    panic!("Failed to add agent: {}", e);
                }
            }
        }

        let vector_fs = Arc::new(VectorFS::new_empty().unwrap());
        let vector_fs_weak = Arc::downgrade(&vector_fs);
        let db_weak = Arc::downgrade(&db);

        let job_manager = Arc::new(Mutex::new(
            JobManager::new(
                db_weak.clone(),
                Arc::clone(&identity_manager),
                clone_signature_secret_key(&identity_secret_key),
                node_profile_name.clone(),
                vector_fs_weak.clone(),
                RemoteEmbeddingGenerator::new_default(),
                UnstructuredAPI::new_default(),
                None,
                None,
            )
            .await,
        ));

        // Add a couple of cron tasks to the database
        {
            match db.add_cron_task(
                node_profile_name.clone(),
                "task1".to_string(),
                "* * * * * * *".to_string(),
                "List all the topics related to AI".to_string(),
                "Summarize this".to_string(),
                "https://news.ycombinator.com".to_string(),
                false,
                agent_id.clone(),
            ) {
                Ok(_) => eprintln!("Added cron task 1"),
                Err(e) => eprintln!("Failed to add cron task: {}", e),
            }
        }

        let db_weak_clone = db_weak.clone();
        let process_job_message_queued_wrapper =
            move |job: CronTask,
                  _db: Weak<ShinkaiDB>,
                  vector_fs_weak: Weak<VectorFS>,
                  identity_sk: SigningKey,
                  job_manager: Arc<Mutex<JobManager>>,
                  node_profile_name: ShinkaiName,
                  profile: String,
                  _ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>| {
                Box::pin(CronManager::process_job_message_queued(
                    job,
                    db_weak_clone.clone(),
                    vector_fs_weak.clone(),
                    identity_sk,
                    job_manager.clone(),
                    node_profile_name.clone(),
                    profile,
                    None,
                )) as Pin<Box<dyn Future<Output = Result<bool, CronManagerError>> + Send>>
            };

        let job_queue_handler = CronManager::process_job_queue(
            db_weak.clone(),
            vector_fs_weak.clone(),
            node_profile_name.clone(),
            clone_signature_secret_key(&identity_secret_key),
            CRON_INTERVAL_TIME,
            job_manager.clone(),
            None,
            process_job_message_queued_wrapper,
        );

        // Set a timeout for the task to complete
        let timeout_duration = Duration::from_millis(5000);
        let job_queue_handler_result = tokio::time::timeout(timeout_duration, job_queue_handler).await;

        // Check the results of the task
        if job_queue_handler_result.is_err() {
            // Handle the error case here if necessary
        }
    }

    #[test]
    fn test_should_execute_cron_task() {
        init_default_tracing();

        use chrono::Timelike;
        use chrono::Utc;

        let cron_task_should_execute = CronTask {
            task_id: "task1".to_string(),
            cron: "* * * * *".to_string(), // This cron task should execute every minute
            prompt: "prompt1".to_string(),
            subprompt: "subprompt1".to_string(),
            url: "url1".to_string(),
            crawl_links: false,
            created_at: Utc::now().to_rfc3339().to_string(),
            llm_provider_id: "agent_id1".to_string(),
        };

        let current_time = Utc::now();
        let next_hour = (current_time.hour() + 2) % 24; // Ensure the next hour is at least 2 hours away

        let cron_task_should_not_execute = CronTask {
            task_id: "task2".to_string(),
            cron: format!("0 {} * * *", next_hour), // This cron task should execute at the start of the next hour
            prompt: "prompt2".to_string(),
            subprompt: "subprompt1".to_string(),
            url: "url2".to_string(),
            crawl_links: false,
            created_at: Utc::now().to_rfc3339().to_string(),
            llm_provider_id: "agent_id2".to_string(),
        };

        let cron_time_interval = 120; // Check if the cron task should execute within the next 2 minutes

        assert!(
            CronManager::should_execute_cron_task(&cron_task_should_execute, cron_time_interval),
            "Expected should_execute_cron_task to return true for a cron task that should execute every minute"
        );

        assert!(
            !CronManager::should_execute_cron_task(&cron_task_should_not_execute, cron_time_interval),
            "Expected should_execute_cron_task to return false for a cron task that should not execute within the next 2 minutes"
        );
    }
}
