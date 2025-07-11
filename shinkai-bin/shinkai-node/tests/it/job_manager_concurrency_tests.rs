use ed25519_dalek::SigningKey;
use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_job_queue_manager::job_queue_manager::{JobForProcessing, JobQueueManager};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_utils::encryption::{
    unsafe_deterministic_encryption_keypair, EncryptionMethod
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::signatures::unsafe_deterministic_signature_keypair;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName, shinkai_message::{
        shinkai_message::ShinkaiMessage, shinkai_message_schemas::{JobMessage, MessageSchemaType}
    }, shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key}
};
use shinkai_node::llm_provider::job_callback_manager::JobCallbackManager;
use shinkai_node::llm_provider::job_manager::JobManager;
use shinkai_node::llm_provider::llm_stopper::LLMStopper;
use shinkai_node::managers::tool_router::ToolRouter;
use shinkai_node::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use shinkai_node::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use shinkai_sqlite::SqliteManager;

use std::result::Result::Ok;
use std::sync::Arc;
use std::sync::Weak;
use std::time::Duration;
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::utils;

fn generate_message_with_text(
    content: String,
    my_encryption_secret_key: EncryptionStaticKey,
    my_signature_secret_key: SigningKey,
    receiver_public_key: EncryptionPublicKey,
    recipient_subidentity_name: String,
    origin_destination_identity_name: String,
    timestamp: String,
) -> ShinkaiMessage {
    let inbox_name = InboxName::get_regular_inbox_name_from_params(
        origin_destination_identity_name.clone().to_string(),
        "".to_string(),
        origin_destination_identity_name.clone().to_string(),
        recipient_subidentity_name.clone().to_string(),
        false,
    )
    .unwrap();

    let inbox_name_value = match inbox_name {
        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
    };

    ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
        .message_raw_content(content.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(MessageSchemaType::TextContent)
        .internal_metadata_with_inbox(
            "".to_string(),
            recipient_subidentity_name.clone().to_string(),
            inbox_name_value,
            EncryptionMethod::None,
            None,
        )
        .external_metadata_with_schedule(
            origin_destination_identity_name.clone().to_string(),
            origin_destination_identity_name.clone().to_string(),
            timestamp,
        )
        .build()
        .unwrap()
}

fn default_test_profile() -> ShinkaiName {
    ShinkaiName::new("@@localhost.shinkai/profileName".to_string()).unwrap()
}

fn node_name() -> ShinkaiName {
    ShinkaiName::new("@@localhost.shinkai".to_string()).unwrap()
}

#[tokio::test]
async fn test_process_job_queue_concurrency() {
    let num_threads = 8;
    let db = utils::db_handlers::setup_test_db();
    let db = Arc::new(db);
    let (node_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
    let node_name = ShinkaiName::new("@@node1.shinkai".to_string()).unwrap();

    // Mock job processing function
    let mock_processing_fn = |job: JobForProcessing,
                              db: Weak<SqliteManager>,
                              _node_name: ShinkaiName,
                              _: SigningKey,
                              _: RemoteEmbeddingGenerator,
                              _: Arc<Mutex<JobCallbackManager>>,
                              _: Arc<Mutex<JobQueueManager<JobForProcessing>>>| {
        Box::pin(async move {
            shinkai_log(
                ShinkaiLogOption::Tests,
                ShinkaiLogLevel::Debug,
                format!("Processing job: {:?}", job.job_message.content).as_str(),
            );
            tokio::time::sleep(Duration::from_millis(200)).await;

            let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
            let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

            // Create a message
            let message = generate_message_with_text(
                job.job_message.content,
                node1_encryption_sk.clone(),
                clone_signature_secret_key(&node1_identity_sk),
                node1_encryption_pk,
                "".to_string(),
                "@@node1.shinkai".to_string(),
                "2023-07-02T20:53:34.812Z".to_string(),
            );

            // Write the message to an inbox with the job name
            let db_arc = db.upgrade().unwrap();
            let _ = db_arc.unsafe_insert_inbox_message(&message.clone(), None, None).await;

            Ok("Success".to_string())
        })
    };

    let db_weak = Arc::downgrade(&db);
    let mut job_queue = JobQueueManager::<JobForProcessing>::new(db_weak.clone(), None)
        .await
        .unwrap();
    let job_queue_manager = Arc::new(Mutex::new(job_queue.clone()));
    let job_queue_manager_immediate = Arc::new(Mutex::new(job_queue.clone()));

    let callback_manager = Arc::new(Mutex::new(JobCallbackManager::new()));
    let llm_stopper = Arc::new(LLMStopper::new());

    // Start processing the queue with concurrency
    let job_queue_handler = JobManager::process_job_queue(
        job_queue_manager.clone(),
        job_queue_manager_immediate.clone(),
        db_weak.clone(),
        node_name.clone(),
        num_threads,
        clone_signature_secret_key(&node_identity_sk),
        RemoteEmbeddingGenerator::new_default(),
        None,
        None,
        callback_manager.clone(),
        None,
        None,
        llm_stopper.clone(),
        move |job: JobForProcessing,
              _db: Weak<SqliteManager>,
              node_name: ShinkaiName,
              identity_sk: SigningKey,
              generator: RemoteEmbeddingGenerator,
              _ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
              _tool_router: Option<Arc<ToolRouter>>,
              _callback_manager: Arc<Mutex<JobCallbackManager>>,
              _job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
              _my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
              _ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
              _llm_stopper: Arc<LLMStopper>| {
            mock_processing_fn(
                job,
                db_weak.clone(),
                node_name.clone(),
                identity_sk,
                generator,
                callback_manager.clone(),
                job_queue_manager.clone(),
            )
        },
    )
    .await;

    // Enqueue multiple jobs
    for i in 0..8 {
        let job = JobForProcessing::new(
            JobMessage {
                job_id: format!("job_id::{}::false", i).to_string(),
                content: format!("my content {}", i).to_string(),
                parent: None,
                sheet_job_data: None,
                callback: None,
                metadata: None,
                tool_key: None,
                fs_files_paths: vec![],
                job_filenames: vec![],
                tools: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
            None,
        );
        job_queue
            .push(format!("job_id::{}::false", i).as_str(), job)
            .await
            .unwrap();
    }

    // Create a new task that lasts at least 2 seconds
    let long_running_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(400)).await;

        let last_messages_all = db.get_last_messages_from_all(10).unwrap();
        assert_eq!(last_messages_all.len(), 8);
    });

    // Set a timeout for both tasks to complete
    let timeout_duration = Duration::from_millis(400);
    let job_queue_handler_result = tokio::time::timeout(timeout_duration, job_queue_handler).await;
    let long_running_task_result = tokio::time::timeout(timeout_duration, long_running_task).await;

    // Check the results of the tasks
    if job_queue_handler_result.is_err() {
        // Handle the error case if necessary
    }

    if long_running_task_result.is_err() {
        // Handle the error case if necessary
    }
}

#[tokio::test]
async fn test_sequential_process_for_same_job_id() {
    let num_threads = 8;
    let db = utils::db_handlers::setup_test_db();
    let db = Arc::new(db);
    let (node_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
    let node_name = ShinkaiName::new("@@node1.shinkai".to_string()).unwrap();

    // Mock job processing function
    let mock_processing_fn = |job: JobForProcessing,
                              db: Weak<SqliteManager>,
                              _node_name: ShinkaiName,
                              _: SigningKey,
                              _: RemoteEmbeddingGenerator,
                              _: Arc<Mutex<JobCallbackManager>>,
                              _: Arc<Mutex<JobQueueManager<JobForProcessing>>>| {
        Box::pin(async move {
            shinkai_log(
                ShinkaiLogOption::Tests,
                ShinkaiLogLevel::Debug,
                format!("Processing job: {:?}", job.job_message.content).as_str(),
            );
            tokio::time::sleep(Duration::from_millis(200)).await;

            let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
            let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

            // Create a message
            let message = generate_message_with_text(
                job.clone().job_message.content,
                node1_encryption_sk.clone(),
                clone_signature_secret_key(&node1_identity_sk),
                node1_encryption_pk,
                "".to_string(),
                "@@node1.shinkai".to_string(),
                "2023-07-02T20:53:34.812Z".to_string(),
            );

            // Write the message to an inbox with the job name
            let db_arc = db.upgrade().unwrap();
            let _ = db_arc.unsafe_insert_inbox_message(&message.clone(), None, None).await;

            Ok("Success".to_string())
        })
    };

    let db_weak = Arc::downgrade(&db);
    let mut job_queue = JobQueueManager::<JobForProcessing>::new(db_weak.clone(), None)
        .await
        .unwrap();
    let job_queue_manager = Arc::new(Mutex::new(job_queue.clone()));
    let job_queue_manager_immediate = Arc::new(Mutex::new(job_queue.clone()));

    let callback_manager = Arc::new(Mutex::new(JobCallbackManager::new()));
    let llm_stopper = Arc::new(LLMStopper::new());

    // Start processing the queue with concurrency
    let job_queue_handler = JobManager::process_job_queue(
        job_queue_manager.clone(),
        job_queue_manager_immediate.clone(),
        db_weak.clone(),
        node_name.clone(),
        num_threads,
        clone_signature_secret_key(&node_identity_sk),
        RemoteEmbeddingGenerator::new_default(),
        None,
        None,
        callback_manager.clone(),
        None,
        None,
        llm_stopper.clone(),
        move |job: JobForProcessing,
              _db: Weak<SqliteManager>,
              node_name: ShinkaiName,
              identity_sk: SigningKey,
              generator: RemoteEmbeddingGenerator,
              _ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
              _tool_router: Option<Arc<ToolRouter>>,
              _callback_manager: Arc<Mutex<JobCallbackManager>>,
              _job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
              _my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
              _ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
              _llm_stopper: Arc<LLMStopper>| {
            mock_processing_fn(
                job,
                db_weak.clone(),
                node_name.clone(),
                identity_sk,
                generator,
                callback_manager.clone(),
                job_queue_manager.clone(),
            )
        },
    )
    .await;

    for i in 0..8 {
        let job = JobForProcessing::new(
            JobMessage {
                job_id: "job_id::123::false".to_string(),
                content: format!("my content {}", i).to_string(),
                parent: None,
                sheet_job_data: None,
                callback: None,
                metadata: None,
                tool_key: None,
                fs_files_paths: vec![],
                job_filenames: vec![],
                tools: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
            None,
        );
        job_queue.push("job_id::123::false", job).await.unwrap();
    }

    // Create a new task that lasts at least 1 seconds
    let db_copy = db.clone();
    let long_running_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;

        let last_messages_all = db_copy.get_last_messages_from_all(10).unwrap();
        assert_eq!(last_messages_all.len(), 1);
    });

    // Set a timeout for both tasks to complete
    let timeout_duration = Duration::from_millis(400);
    let job_queue_handler_result = tokio::time::timeout(timeout_duration, job_queue_handler).await;
    let long_running_task_result = tokio::time::timeout(timeout_duration, long_running_task).await;

    // Check the results of the tasks
    if job_queue_handler_result.is_err() {
        // Handle the error case if necessary
    }

    if long_running_task_result.is_err() {
        // Handle the error case if necessary
    }
}
