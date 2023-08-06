use async_channel::{bounded, Receiver, Sender};
use shinkai_node::managers::IdentityManager;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mockito::Server;
    use shinkai_message_wasm::{
        schemas::{inbox_name::InboxName, shinkai_name::ShinkaiName},
        shinkai_message::shinkai_message_schemas::JobScope,
        shinkai_utils::{
            encryption::unsafe_deterministic_encryption_keypair,
            shinkai_message_builder::ShinkaiMessageBuilder,
            signatures::{clone_signature_secret_key, unsafe_deterministic_signature_keypair},
            utils::hash_string,
        },
    };
    use shinkai_node::{
        db::ShinkaiDB,
        managers::{
            agent::{Agent, AgentAPIModel},
            agent_serialization::SerializedAgent,
            identity_manager,
            job_manager::{AgentManager, JobLike, JobManager},
            providers::openai::OpenAI,
        },
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::{mpsc, Mutex};

    #[tokio::test]
    async fn test_process_job_message_creation() {
        setup();
        let node_profile_name = ShinkaiName::new("@@node1.shinkai".to_string()).unwrap();

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let mut server = Server::new();
        let _m = server
            .mock("POST", "/v1/chat/completions")
            .match_header("authorization", "Bearer mockapikey")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "id": "chatcmpl-123",
                "object": "chat.completion",
                "created": 1677652288,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "\n\nHello there, how may I assist you today?"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 9,
                    "completion_tokens": 12,
                    "total_tokens": 21
                }
            }"#,
            )
            .create();

        let db_path = format!("db_tests/{}", hash_string("agent_test".clone()));
        let mut db = ShinkaiDB::new(&db_path).unwrap();

        let db_arc = Arc::new(Mutex::new(db));
        {
            let db_lock = db_arc.lock().await;
            match db_lock.update_local_node_keys(
                node_profile_name.clone(),
                node1_encryption_pk.clone(),
                node1_identity_pk.clone(),
            ) {
                Ok(_) => (),
                Err(e) => panic!("Failed to update local node keys: {}", e),
            }
        }
        let subidentity_manager = IdentityManager::new(db_arc.clone(), node_profile_name.clone())
            .await
            .unwrap();
        let identity_manager = Arc::new(Mutex::new(subidentity_manager));

        // Create an agent
        let openai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };

        let agent = SerializedAgent {
            id: "test_agent_id".to_string(),
            full_identity_name: ShinkaiName::from_node_and_profile(
                node_profile_name.get_node_name(),
                "test_name".to_string(),
            )
            .unwrap(),
            perform_locally: false,
            external_url: Some(server.url()),
            api_key: Some("mockapikey".to_string()),
            model: AgentAPIModel::OpenAI(openai),
            toolkit_permissions: vec!["toolkit1".to_string(), "toolkit2".to_string()],
            storage_bucket_permissions: vec!["storage1".to_string(), "storage2".to_string()],
            allowed_message_senders: vec!["sender1".to_string(), "sender2".to_string()],
        };
        {
            let mut db = db_arc.lock().await;
            db.add_agent(agent.clone());
            let _ = identity_manager.lock().await.add_agent_subidentity(agent.clone()).await;
        }

        // Create JobManager
        let mut job_manager = JobManager::new(db_arc.clone(), identity_manager).await;

        // Create a JobCreationMessage ShinkaiMessage
        let scope = JobScope {
            buckets: vec![InboxName::new("inbox::@@node1.shinkai|test_name::@@|::false".to_string()).unwrap()],
            documents: vec!["document1".to_string(), "document2".to_string()],
        };
        let shinkai_message_creation = ShinkaiMessageBuilder::job_creation(
            scope,
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_pk.clone(),
            node_profile_name.to_string().clone(),
            node_profile_name.to_string(),
            agent.id.clone(),
        )
        .unwrap();

        // Process the JobCreationSchema message
        let mut job_created_id = String::new();
        match job_manager.process_job_message(shinkai_message_creation, None).await {
            Ok(job_id) => {
                job_created_id = job_id.clone();
                println!("Job ID: {}", job_id);
                // Check that the job was created correctly
                let retrieved_job = db_arc.clone().lock().await.get_job(&job_id);
                assert!(retrieved_job.is_ok());
            }
            Err(e) => {
                panic!("Unexpected error processing job message: {}", e);
            }
        }

        //
        // Second part of the test after job is created trilogy
        //

        let shinkai_job_message = ShinkaiMessageBuilder::job_message(
            job_created_id.clone(),
            "hello?".to_string(),
            node1_encryption_sk,
            node1_identity_sk,
            node1_encryption_pk,
            node_profile_name.to_string().clone(),
            node_profile_name.to_string(),
            agent.id,
        )
        .unwrap();

        match job_manager.process_job_message(shinkai_job_message, None).await {
            Ok(job_id) => {
                job_created_id = job_id.clone();
                println!("Job Message ID: {}", job_id);
                // Check that the job was created correctly
                let retrieved_job = db_arc.clone().lock().await.get_job(&job_id);
                assert!(retrieved_job.is_ok());
            }
            Err(e) => {
                panic!("Unexpected error processing job message: {}", e);
            }
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}
