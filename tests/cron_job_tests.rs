#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
    use futures::Future;
    use shinkai_message_primitives::{
        schemas::shinkai_name::ShinkaiName,
        shinkai_utils::signatures::{clone_signature_secret_key, unsafe_deterministic_signature_keypair},
    };
    use shinkai_node::{
        agent::queue::job_queue_manager::JobQueueManager,
        cron_tasks::cron_manager::{CronManager, CronManagerError},
        db::{db_cron_task::CronTask, ShinkaiDB},
    };
    use std::{fs, path::Path, pin::Pin, sync::Arc, time::Duration};
    use tokio::sync::Mutex;
    use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

    const NUM_THREADS: usize = 4;
    const CRON_INTERVAL_TIME: u64 = 60 * 10; // it doesn't matter here

    #[test]
    fn setup() {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(&path);
    }

    #[tokio::test]
    async fn test_process_cron_job_queue() {
        setup();
        eprintln!("test_process_cron_job_queue");
        let db = Arc::new(Mutex::new(ShinkaiDB::new("db_tests/").unwrap()));
        let (identity_secret_key, _) = unsafe_deterministic_signature_keypair(0);
        let node_profile_name = ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap();

        // Add a couple of cron tasks to the database
        {
            let mut db_lock = db.lock().await;
            db_lock
                .add_cron_task(
                    node_profile_name.clone().get_profile_name().unwrap().to_string(),
                    "task1".to_string(),
                    "cron1".to_string(),
                    "prompt1".to_string(),
                    "url1".to_string(),
                )
                .unwrap();

            db_lock
                .add_cron_task(
                    node_profile_name.clone().get_profile_name().unwrap().to_string(),
                    "task2".to_string(),
                    "cron2".to_string(),
                    "prompt2".to_string(),
                    "url2".to_string(),
                )
                .unwrap();
        }

        let process_job_message_queued_wrapper =
            |job: CronTask, db: Arc<Mutex<ShinkaiDB>>, identity_sk: SignatureStaticKey| {
                Box::pin(CronManager::process_job_message_queued(job, db, identity_sk))
                    as Pin<Box<dyn Future<Output = Result<bool, CronManagerError>> + Send>>
            };

        let job_queue_handler = CronManager::process_job_queue(
            db.clone(),
            node_profile_name.clone(),
            clone_signature_secret_key(&identity_secret_key),
            CRON_INTERVAL_TIME,
            process_job_message_queued_wrapper,
        );

        // Set a timeout for the task to complete
        let timeout_duration = Duration::from_millis(400);
        let job_queue_handler_result = tokio::time::timeout(timeout_duration, job_queue_handler).await;

        // Check the results of the task
        match job_queue_handler_result {
            Ok(_) => (),
            Err(_) => (),
        }
    }
}
