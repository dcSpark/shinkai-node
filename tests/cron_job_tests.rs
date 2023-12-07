#[cfg(test)]
mod tests {
    use std::env;
    use core::panic;
    use ed25519_dalek::SigningKey;
    use futures::Future;
    use shinkai_message_primitives::{
        schemas::{
            agents::serialized_agent::{AgentLLMInterface, OpenAI, SerializedAgent},
            shinkai_name::ShinkaiName,
        },
        shinkai_utils::{
            encryption::unsafe_deterministic_encryption_keypair,
            signatures::{clone_signature_secret_key, unsafe_deterministic_signature_keypair},
        },
    };
    use shinkai_node::{
        agent::job_manager::JobManager,
        cron_tasks::cron_manager::{CronManager, CronManagerError},
        db::{db_cron_task::CronTask, ShinkaiDB},
        managers::IdentityManager,
    };
    use std::{fs, path::Path, pin::Pin, sync::Arc, time::Duration};
    use tokio::sync::Mutex;

    const NUM_THREADS: usize = 1;
    const CRON_INTERVAL_TIME: u64 = 60 * 10; // it doesn't matter here

    #[test]
    fn setup() {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(&path);
    }

    #[tokio::test]
    async fn test_process_cron_job() {
        setup();
        let db = Arc::new(Mutex::new(ShinkaiDB::new("db_tests/").unwrap()));
        let (identity_secret_key, identity_public_key) = unsafe_deterministic_signature_keypair(0);
        let (_, encryption_public_key) = unsafe_deterministic_encryption_keypair(0);
        let node_profile_name = ShinkaiName::new("@@localhost.shinkai/main".to_string()).unwrap();
        let agent_id = "agent_id1".to_string();
        let agent_name =
            ShinkaiName::new(format!("{}/agent/{}", node_profile_name.clone(), agent_id.clone()).to_string()).unwrap();

        {
            // add keys
            let db_lock = db.lock().await;
            match db_lock.update_local_node_keys(
                node_profile_name.clone(),
                encryption_public_key.clone(),
                identity_public_key.clone(),
            ) {
                Ok(_) => (),
                Err(e) => panic!("Failed to update local node keys: {}", e),
            }
        }

        let subidentity_manager = IdentityManager::new(db.clone(), node_profile_name.clone())
            .await
            .unwrap();
        let identity_manager = Arc::new(Mutex::new(subidentity_manager));

        {
            let mut db_lock = db.lock().await;

            let open_ai = OpenAI {
                model_type: "gpt-3.5-turbo-1106".to_string(),
            };

            let agent = SerializedAgent {
                id: agent_id.clone(),
                full_identity_name: agent_name,
                perform_locally: false,
                external_url: Some("https://api.openai.com".to_string()),
                api_key: env::var("INITIAL_AGENT_API_KEY").ok(),
                model: AgentLLMInterface::OpenAI(open_ai),
                toolkit_permissions: vec![],
                storage_bucket_permissions: vec![],
                allowed_message_senders: vec![],
            };

            // add agent
            match db_lock.add_agent(agent.clone()) {
                Ok(()) => {
                    let mut subidentity_manager = identity_manager.lock().await;
                    match subidentity_manager.add_agent_subidentity(agent).await {
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

        let job_manager = Arc::new(Mutex::new(
            JobManager::new(
                Arc::clone(&db),
                Arc::clone(&identity_manager),
                clone_signature_secret_key(&identity_secret_key),
                node_profile_name.clone(),
            )
            .await,
        ));

        // Add a couple of cron tasks to the database
        {
            let mut db_lock = db.lock().await;
            match db_lock.add_cron_task(
                node_profile_name.clone(),
                "task1".to_string(),
                "* * * * * * *".to_string(),
                "List all the topics related to AI".to_string(),
                "Summarize this".to_string(),
                "https://news.ycombinator.com".to_string(),
                false,
                agent_id.clone(),
            ) {
                Ok(_) => (),
                Err(e) => eprintln!("Failed to add cron task: {}", e),
            }
        }

        let process_job_message_queued_wrapper =
            |job: CronTask,
             db: Arc<Mutex<ShinkaiDB>>,
             identity_sk: SigningKey,
             job_manager: Arc<Mutex<JobManager>>,
             node_profile_name: ShinkaiName,
             profile: String | {
                Box::pin(CronManager::process_job_message_queued(
                    job,
                    db,
                    identity_sk,
                    job_manager.clone(),
                    node_profile_name.clone(),
                    profile,
                )) as Pin<Box<dyn Future<Output = Result<bool, CronManagerError>> + Send>>
            };

        let job_queue_handler = CronManager::process_job_queue(
            db.clone(),
            node_profile_name.clone(),
            clone_signature_secret_key(&identity_secret_key),
            CRON_INTERVAL_TIME,
            job_manager.clone(),
            process_job_message_queued_wrapper,
        );

        // Set a timeout for the task to complete
        let timeout_duration = Duration::from_millis(100000);
        let job_queue_handler_result = tokio::time::timeout(timeout_duration, job_queue_handler).await;

        // Check the results of the task
        match job_queue_handler_result {
            Ok(_) => (),
            Err(_) => (),
        }
    }

    #[test]
    fn test_should_execute_cron_task() {
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
            agent_id: "agent_id1".to_string(),
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
            agent_id: "agent_id2".to_string(),
        };

        let cron_time_interval = 120; // Check if the cron task should execute within the next 2 minutes

        assert_eq!(
            CronManager::should_execute_cron_task(&cron_task_should_execute, cron_time_interval),
            true,
            "Expected should_execute_cron_task to return true for a cron task that should execute every minute"
        );

        assert_eq!(
        CronManager::should_execute_cron_task(&cron_task_should_not_execute, cron_time_interval),
        false,
        "Expected should_execute_cron_task to return false for a cron task that should not execute within the next 2 minutes"
    );
    }
}
