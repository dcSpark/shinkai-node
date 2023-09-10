use async_channel::{bounded, Receiver, Sender};
use shinkai_message_wasm::schemas::agents::serialized_agent::{AgentAPIModel, OpenAI, SerializedAgent};
use shinkai_message_wasm::schemas::inbox_name::InboxName;
use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;
use shinkai_message_wasm::schemas::shinkai_time::ShinkaiTime;
use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::{JobMessage, MessageSchemaType};
use shinkai_message_wasm::shinkai_utils::encryption::{
    clone_static_secret_key, unsafe_deterministic_encryption_keypair, EncryptionMethod,
};
use shinkai_message_wasm::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_wasm::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_message_wasm::shinkai_utils::utils::hash_string;
use shinkai_node::managers::agent;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::node_api::APIError;
use shinkai_node::network::Node;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

use crate::utils::node_test_api::{
    api_agent_registration, api_create_job, api_message_job, api_registration_device_node_profile_main,
};

mod utils;
use mockito::Server;

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn node_agent_registration() {
    // WIP: need to find a way to test the agent registration
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.shinkai";
        let node1_subidentity_name = "main";
        let node1_device_name = "node1_device";
        let node1_agent = "node1_gpt_agent";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let (node1_profile_identity_sk, node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node1_device_identity_sk, node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk.clone(),
            0,
            node1_commands_receiver,
            node1_db_path,
        );

        println!("Starting Node");
        let node1_handler = tokio::spawn(async move {
            println!("\n\n");
            println!("Starting node 1");
            let _ = node1.await.start().await;
        });

        let interactions_handler = tokio::spawn(async move {
            println!("Registration of an Admin Profile");

            {
                // Register a Profile in Node1 and verifies it
                eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
                api_registration_device_node_profile_main(
                    node1_commands_sender.clone(),
                    node1_subidentity_name,
                    node1_identity_name,
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_device_identity_sk),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_device_name,
                )
                .await;
            }

            let mut server = Server::new();
            {
                // Register an Agent
                eprintln!("\n\nRegister an Agent in Node1 and verify it");
                let open_ai = OpenAI {
                    model_type: "gpt-3.5-turbo".to_string(),
                };
                let agent_name = ShinkaiName::new(
                    format!(
                        "{}/{}/agent/{}",
                        node1_identity_name.clone(),
                        node1_subidentity_name.clone(),
                        node1_agent.clone()
                    )
                    .to_string(),
                )
                .unwrap();

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

                let agent = SerializedAgent {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    perform_locally: false,
                    external_url: Some(server.url()),
                    api_key: Some("mockapikey".to_string()),
                    model: AgentAPIModel::OpenAI(open_ai),
                    toolkit_permissions: vec![],
                    storage_bucket_permissions: vec![],
                    allowed_message_senders: vec![],
                };
                api_agent_registration(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone(),
                    node1_subidentity_name.clone(),
                    agent,
                )
                .await;
            }

            let mut job_id = "".to_string();
            let agent_subidentity =
                format!("{}/agent/{}", node1_subidentity_name.clone(), node1_agent.clone()).to_string();
            {
                // Create a Job
                eprintln!("\n\nCreate a Job for the previous Agent in Node1 and verify it");
                job_id = api_create_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone(),
                    node1_subidentity_name.clone(),
                    &agent_subidentity.clone(),
                )
                .await;
            }

            {
                // Send a Message to the Job for processing
                eprintln!("\n\nSend a message for a Job");
                let message = "Tell me. Who are you?".to_string();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone(),
                    node1_subidentity_name.clone(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &message,
                )
                .await;
            }
            {
                // Successfully read job inbox
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
                let sender = format!("{}/{}", node1_identity_name.clone(), node1_subidentity_name.clone());

                let msg = ShinkaiMessageBuilder::get_last_messages_from_inbox(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    inbox_name.to_string(),
                    10,
                    None,
                    "".to_string(),
                    sender,
                    node1_identity_name.clone().to_string(),
                )
                .unwrap();
                let (res2_sender, res2_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIGetLastMessagesFromInbox { msg, res: res2_sender })
                    .await
                    .unwrap();
                let node2_last_messages = res2_receiver.recv().await.unwrap().expect("Failed to receive messages");
                println!("### node2_last_messages: {:?}", node2_last_messages);
                let shinkai_message_content_agent = node2_last_messages[1].get_message_content().unwrap();
                let message_content_agent: JobMessage = serde_json::from_str(&shinkai_message_content_agent).unwrap();

                assert_eq!(
                    message_content_agent.content,
                    "\n\nHello there, how may I assist you today?".to_string()
                );
                assert!(node2_last_messages.len() == 2);
            }
            {
                // Check Profile inboxes (to confirm job's there)
                let sender = format!("{}/{}", node1_identity_name.clone(), node1_subidentity_name.clone());
                eprintln!("sender: {}", sender);

                let msg = ShinkaiMessageBuilder::get_all_inboxes_for_profile(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    sender.clone().to_string(),
                    "".to_string(),
                    sender,
                    node1_identity_name.clone().to_string(),
                )
                .unwrap();

                let (res2_sender, res2_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIGetAllInboxesForProfile { msg, res: res2_sender })
                    .await
                    .unwrap();
                let node2_last_messages = res2_receiver.recv().await.unwrap().expect("Failed to receive messages");
                println!("node1_all_profiles: {:?}", node2_last_messages);
                assert!(node2_last_messages.len() == 1);
            }

            // Now we add more messages to properly test unread and pagination
            {
                // Send a Message to the Job for processing
                let message = "Are you still there?".to_string();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone(),
                    node1_subidentity_name.clone(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &message,
                )
                .await;

                // Successfully read unread messages from job inbox
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
                let sender = format!("{}/{}", node1_identity_name.clone(), node1_subidentity_name.clone());

                let msg = ShinkaiMessageBuilder::get_last_unread_messages_from_inbox(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    inbox_name.to_string(),
                    3,
                    None,
                    "".to_string(),
                    sender.clone(),
                    node1_identity_name.clone().to_string(),
                )
                .unwrap();
                let (res2_sender, res2_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIGetLastUnreadMessagesFromInbox { msg, res: res2_sender })
                    .await
                    .unwrap();
                let node2_last_messages = res2_receiver.recv().await.unwrap().expect("Failed to receive messages");
                println!("### node2_last_messages: {:?}", node2_last_messages);
                let shinkai_message_content_agent = node2_last_messages[2].get_message_content().unwrap();
                let message_content_agent: JobMessage = serde_json::from_str(&shinkai_message_content_agent).unwrap();

                assert_eq!(message_content_agent.content, message.to_string());
                assert!(node2_last_messages.len() == 3);

                let offset = format!(
                    "{}:::{}",
                    node2_last_messages[1].external_metadata.scheduled_time,
                    node2_last_messages[1].calculate_message_hash()
                );
                eprintln!("next_msg offset: {}", offset);
                let next_msg = ShinkaiMessageBuilder::get_last_unread_messages_from_inbox(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    inbox_name.to_string(),
                    4,
                    Some(offset.clone()),
                    "".to_string(),
                    sender.clone(),
                    node1_identity_name.clone().to_string(),
                )
                .unwrap();
                let (res2_sender, res2_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIGetLastUnreadMessagesFromInbox {
                        msg: next_msg,
                        res: res2_sender,
                    })
                    .await
                    .unwrap();
                let node2_last_messages = res2_receiver.recv().await.unwrap().expect("Failed to receive messages");
                println!("### node2_last_messages unread pagination: {:?}", node2_last_messages);
                let shinkai_message_content_agent = node2_last_messages[0].get_message_content().unwrap();
                let message_content_agent: JobMessage = serde_json::from_str(&shinkai_message_content_agent).unwrap();

                assert!(node2_last_messages.len() == 2);
                assert_eq!(message_content_agent.content, message.to_string());

                // we mark read until the offset
                let read_msg = ShinkaiMessageBuilder::read_up_to_time(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    inbox_name.to_string(),
                    offset,
                    "".to_string(),
                    sender,
                    node1_identity_name.clone().to_string(),
                )
                .unwrap();
                let (res2_sender, res2_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIMarkAsReadUpTo {
                        msg: read_msg,
                        res: res2_sender,
                    })
                    .await
                    .unwrap();
            }
            {
                // check how many unread messages are left
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
                let sender = format!("{}/{}", node1_identity_name.clone(), node1_subidentity_name.clone());

                let msg = ShinkaiMessageBuilder::get_last_unread_messages_from_inbox(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    inbox_name.to_string(),
                    3,
                    None,
                    "".to_string(),
                    sender.clone(),
                    node1_identity_name.clone().to_string(),
                )
                .unwrap();
                let (res2_sender, res2_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIGetLastUnreadMessagesFromInbox { msg, res: res2_sender })
                    .await
                    .unwrap();
                let node2_last_messages = res2_receiver.recv().await.unwrap().expect("Failed to receive messages");
                println!(
                    "### unread after cleaning node2_last_messages: {:?}",
                    node2_last_messages
                );
                eprintln!(
                    "### unread after cleaning node2_last_messages len: {:?}",
                    node2_last_messages.len()
                );
                assert!(node2_last_messages.len() == 2);
            }
            {
                // Send a scheduled message
                let message = "scheduled message".to_string();
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
                let sender = format!("{}/{}", node1_identity_name.clone(), node1_subidentity_name.clone());
                let future_time_2_secs = ShinkaiTime::generate_time_in_future_with_secs(2);

                let msg = ShinkaiMessageBuilder::new(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                )
                .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                .external_metadata_with_schedule(node1_identity_name.clone().to_string(), sender, future_time_2_secs)
                .message_raw_content(message.clone())
                .internal_metadata_with_inbox("".to_string(), "".to_string(), inbox_name.to_string(), EncryptionMethod::None)
                .build();
            }
        });

        // Wait for all tasks to complete
        let _ = tokio::try_join!(node1_handler, interactions_handler).unwrap();
    });
}
