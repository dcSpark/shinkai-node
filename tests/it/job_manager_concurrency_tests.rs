use ed25519_dalek::{SigningKey, VerifyingKey};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::shinkai_utils::encryption::{
    unsafe_deterministic_encryption_keypair, EncryptionMethod,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{
    init_default_tracing, shinkai_log, ShinkaiLogLevel, ShinkaiLogOption,
};
use shinkai_message_primitives::shinkai_utils::signatures::unsafe_deterministic_signature_keypair;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{JobMessage, MessageSchemaType},
    },
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use shinkai_node::agent::job_manager::JobManager;
use shinkai_node::agent::queue::job_queue_manager::{JobForProcessing, JobQueueManager};
use shinkai_node::db::{ShinkaiDB, Topic};
use shinkai_node::vector_fs::vector_fs::VectorFS;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::model_type::{EmbeddingModelType, TextEmbeddingsInference};
use shinkai_vector_resources::unstructured::unstructured_api::UnstructuredAPI;
use std::result::Result::Ok;
use std::sync::Weak;
use std::time::{Duration, Instant};
use std::{collections::HashMap, error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex, Semaphore};
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

    let message = ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
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
        .unwrap();
    message
}

fn default_test_profile() -> ShinkaiName {
    ShinkaiName::new("@@localhost.shinkai/profileName".to_string()).unwrap()
}

fn node_name() -> ShinkaiName {
    ShinkaiName::new("@@localhost.shinkai".to_string()).unwrap()
}

fn setup_default_vector_fs() -> VectorFS {
    let generator = RemoteEmbeddingGenerator::new_default();
    let fs_db_path = format!("db_tests/{}", "vector_fs");
    let profile_list = vec![default_test_profile()];
    let supported_embedding_models = vec![EmbeddingModelType::TextEmbeddingsInference(
        TextEmbeddingsInference::AllMiniLML6v2,
    )];

    VectorFS::new(
        generator,
        supported_embedding_models,
        profile_list,
        &fs_db_path,
        node_name(),
    )
    .unwrap()
}

#[tokio::test]
async fn test_process_job_queue_concurrency() {
    init_default_tracing();
    utils::db_handlers::setup();

    let NUM_THREADS = 8;
    let db_path = "db_tests/";
    let db = Arc::new(Mutex::new(ShinkaiDB::new(db_path).unwrap()));
    let vector_fs = Arc::new(Mutex::new(setup_default_vector_fs()));
    let (node_identity_sk, _) = unsafe_deterministic_signature_keypair(0);

    // Mock job processing function
    let mock_processing_fn = |job: JobForProcessing,
                              db: Weak<Mutex<ShinkaiDB>>,
                              vector_fs: Weak<Mutex<VectorFS>>,
                              _: SigningKey,
                              _: RemoteEmbeddingGenerator,
                              _: UnstructuredAPI| {
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
            let mut db = db_arc.lock().await;
            let _ = db.unsafe_insert_inbox_message(&message.clone(), None).await;

            Ok("Success".to_string())
        })
    };

    let db_weak = Arc::downgrade(&db);
    let vector_fs_weak = Arc::downgrade(&vector_fs);
    let mut job_queue = JobQueueManager::<JobForProcessing>::new(db_weak.clone(), Topic::JobQueues.as_str()).await.unwrap();
    let job_queue_manager = Arc::new(Mutex::new(job_queue.clone()));

    // Start processing the queue with concurrency
    let job_queue_handler = JobManager::process_job_queue(
        job_queue_manager,
        db_weak.clone(),
        vector_fs_weak.clone(),
        NUM_THREADS,
        clone_signature_secret_key(&node_identity_sk),
        RemoteEmbeddingGenerator::new_default(),
        UnstructuredAPI::new_default(),
        move |job, db, vector_fs, identity_sk, generator, unstructured_api| {
            mock_processing_fn(
                job,
                db_weak.clone(),
                vector_fs_weak.clone(),
                identity_sk,
                generator,
                unstructured_api,
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
                files_inbox: "".to_string(),
                parent: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        );
        job_queue
            .push(format!("job_id::{}::false", i).as_str(), job)
            .await
            .unwrap();
    }

    // Create a new task that lasts at least 2 seconds
    let long_running_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(400)).await;

        let last_messages_all = db.lock().await.get_last_messages_from_all(10).unwrap();
        assert_eq!(last_messages_all.len(), 8);
    });

    // Set a timeout for both tasks to complete
    let timeout_duration = Duration::from_millis(400);
    let job_queue_handler_result = tokio::time::timeout(timeout_duration, job_queue_handler).await;
    let long_running_task_result = tokio::time::timeout(timeout_duration, long_running_task).await;

    // Check the results of the tasks
    match job_queue_handler_result {
        Ok(_) => (),
        Err(_) => (),
    }

    match long_running_task_result {
        Ok(_) => (),
        Err(_) => (),
    }
}

#[tokio::test]
async fn test_sequential_process_for_same_job_id() {
    init_default_tracing();
    super::utils::db_handlers::setup();

    let NUM_THREADS = 8;
    let db_path = "db_tests/";
    let db = Arc::new(Mutex::new(ShinkaiDB::new(db_path).unwrap()));
    let vector_fs = Arc::new(Mutex::new(setup_default_vector_fs()));
    let (node_identity_sk, _) = unsafe_deterministic_signature_keypair(0);

    // Mock job processing function
    let mock_processing_fn = |job: JobForProcessing,
                              db: Weak<Mutex<ShinkaiDB>>,
                              vector_fs: Weak<Mutex<VectorFS>>,
                              _: SigningKey,
                              _: RemoteEmbeddingGenerator,
                              _: UnstructuredAPI| {
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
            let mut db = db_arc.lock().await;
            let _ = db.unsafe_insert_inbox_message(&message.clone(), None).await;

            Ok("Success".to_string())
        })
    };

    let db_weak = Arc::downgrade(&db);
    let vector_fs_weak = Arc::downgrade(&vector_fs);
    let mut job_queue = JobQueueManager::<JobForProcessing>::new(db_weak.clone(), Topic::JobQueues.as_str()).await.unwrap();
    let job_queue_manager = Arc::new(Mutex::new(job_queue.clone()));

    // Start processing the queue with concurrency
    let job_queue_handler = JobManager::process_job_queue(
        job_queue_manager,
        db_weak.clone(),
        vector_fs_weak.clone(),
        NUM_THREADS,
        clone_signature_secret_key(&node_identity_sk),
        RemoteEmbeddingGenerator::new_default(),
        UnstructuredAPI::new_default(),
        move |job, db, vector_fs, identity_sk, generator, unstructured_api| {
            mock_processing_fn(
                job,
                db_weak.clone(),
                vector_fs_weak.clone(),
                identity_sk,
                generator,
                unstructured_api,
            )
        },
    )
    .await;

    for i in 0..8 {
        let job = JobForProcessing::new(
            JobMessage {
                job_id: "job_id::123::false".to_string(),
                content: format!("my content {}", i).to_string(),
                files_inbox: "".to_string(),
                parent: None,
            },
            ShinkaiName::new("@@node1.shinkai/main".to_string()).unwrap(),
        );
        job_queue.push("job_id::123::false", job).await.unwrap();
    }

    // Create a new task that lasts at least 1 seconds
    let db_copy = db.clone();
    let long_running_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;

        let last_messages_all = db_copy.lock().await.get_last_messages_from_all(10).unwrap();
        assert_eq!(last_messages_all.len(), 1);
    });

    // Set a timeout for both tasks to complete
    let timeout_duration = Duration::from_millis(400);
    let job_queue_handler_result = tokio::time::timeout(timeout_duration, job_queue_handler).await;
    let long_running_task_result = tokio::time::timeout(timeout_duration, long_running_task).await;

    // Check the results of the tasks
    match job_queue_handler_result {
        Ok(_) => (),
        Err(_) => (),
    }

    match long_running_task_result {
        Ok(_) => (),
        Err(_) => (),
    }

    let _ = db.lock().await;
}
