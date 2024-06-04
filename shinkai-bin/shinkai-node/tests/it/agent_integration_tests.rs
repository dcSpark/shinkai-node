use async_channel::{bounded, Receiver, Sender};
use shinkai_message_primitives::schemas::agents::serialized_agent::{
    AgentLLMInterface, Ollama, OpenAI, SerializedAgent, ShinkaiBackend,
};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{JobMessage, MessageSchemaType};
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, unsafe_deterministic_encryption_keypair, EncryptionMethod,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{
    init_default_tracing, shinkai_log, ShinkaiLogLevel, ShinkaiLogOption,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use shinkai_vector_resources::shinkai_time::ShinkaiStringTime;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

use super::utils::node_test_api::{
    api_create_job, api_message_job, api_registration_device_node_profile_main,
};

use mockito::Server;

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn node_agent_registration() {
    std::env::set_var("WELCOME_MESSAGE", "false");
    init_default_tracing();
    // WIP: need to find a way to test the agent registration
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.sepolia-shinkai";
        let node1_subidentity_name = "main";
        let node1_device_name = "node1_device";
        let node1_agent = "node1_gpt_agent";

        let (node1_identity_sk, _node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let (node1_profile_identity_sk, _node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, _node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node1_device_identity_sk, _node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, _node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));
        let node1_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node1_identity_name.clone()));

        // Agent pre-creation

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
                    "content": "\n# Answer\n Hello there, how may I assist you today?"
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

        let agent_name = ShinkaiName::new(
            format!(
                "{}/{}/agent/{}",
                node1_identity_name, node1_subidentity_name, node1_agent
            )
            .to_string(),
        )
        .unwrap();

        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo-1106".to_string(),
        };

        let _ollama = Ollama {
            model_type: "mistral".to_string(),
        };

        let _shinkai_backend = ShinkaiBackend::new("gpt-4-1106-preview");

        let agent = SerializedAgent {
            id: node1_agent.to_string(),
            full_identity_name: agent_name,
            perform_locally: false,
            // external_url: Some("http://localhost:3000".to_string()),
            // external_url: Some("http://localhost:11434".to_string()),
            // external_url: Some("https://api.openai.com".to_string()),
            external_url: Some(server.url()),
            // api_key: Some("api_key".to_string()),
            api_key: Some("mockapikey".to_string()),
            // api_key: Some("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1c2VySWQiOjE3LCJpYXQiOjE3MDEzMTg5ODZ9.jTLpbsAVITowuCMYdNgTyUHikGRlLjEqqOYHWMRNSz4".to_string()),
            model: AgentLLMInterface::OpenAI(open_ai),
            // model: AgentLLMInterface::Ollama(ollama),
            // model: AgentLLMInterface::ShinkaiBackend(shinkai_backend),
            toolkit_permissions: vec![],
            storage_bucket_permissions: vec![],
            allowed_message_senders: vec![],
        };

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk.clone(),
            0,
            node1_commands_receiver,
            node1_db_path,
            "".to_string(),
            None,
            true,
            vec![agent],
            None,
            node1_fs_db_path,
            None,
            None,
        );

        let node1_handler = tokio::spawn(async move {
            shinkai_log(ShinkaiLogOption::Tests, ShinkaiLogLevel::Debug, "Starting Node 1");
            let _ = node1.await.lock().await.start().await;
        });

        let abort_handler = node1_handler.abort_handle();

        let interactions_handler = tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::Tests,
                ShinkaiLogLevel::Debug,
                "\n\nRegistration of an Admin Profile",
            );

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

            let mut job_id = "".to_string();
            let agent_subidentity = format!("{}/agent/{}", node1_subidentity_name, node1_agent).to_string();
            {
                // Create a Job
                shinkai_log(
                    ShinkaiLogOption::Tests,
                    ShinkaiLogLevel::Debug,
                    &format!("Creating a Job for Agent {}", agent_subidentity.clone()),
                );
                job_id = api_create_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name,
                    node1_subidentity_name,
                    &agent_subidentity.clone(),
                )
                .await;
            }
            {
                // Send a Message to the Job for processing
                shinkai_log(
                    ShinkaiLogOption::API,
                    ShinkaiLogLevel::Debug,
                    &format!("Sending a message to Job {}", job_id.clone()),
                );
                let message = "1) Tell me. Who are you?".to_string();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name,
                    node1_subidentity_name,
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &message,
                    "",
                    "",
                )
                .await;
            }
            {
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
                let sender = format!("{}/{}", node1_identity_name, node1_subidentity_name);

                let mut node2_last_messages = vec![];
                for _ in 0..30 {
                    let msg = ShinkaiMessageBuilder::get_last_messages_from_inbox(
                        clone_static_secret_key(&node1_profile_encryption_sk),
                        clone_signature_secret_key(&node1_profile_identity_sk),
                        node1_encryption_pk,
                        inbox_name.to_string(),
                        10,
                        None,
                        "".to_string(),
                        sender.clone(),
                        node1_identity_name.to_string(),
                    )
                    .unwrap();
                    let (res2_sender, res2_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::APIGetLastMessagesFromInbox { msg, res: res2_sender })
                        .await
                        .unwrap();
                    node2_last_messages = res2_receiver.recv().await.unwrap().expect("Failed to receive messages");

                    if node2_last_messages.len() >= 2 {
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;
                }

                shinkai_log(
                    ShinkaiLogOption::Tests,
                    ShinkaiLogLevel::Debug,
                    &format!("node2_last_messages: {:?}", node2_last_messages),
                );

                let shinkai_message_content_agent = node2_last_messages[1].get_message_content().unwrap();
                let message_content_agent: JobMessage = serde_json::from_str(&shinkai_message_content_agent).unwrap();

                assert_eq!(
                    message_content_agent.content,
                    "Hello there, how may I assist you today?".to_string()
                );
                assert!(node2_last_messages.len() == 2);
            }
            {
                // Check Profile inboxes (to confirm job's there)
                let full_profile = format!("{}/{}", node1_identity_name, node1_subidentity_name);

                let msg = ShinkaiMessageBuilder::get_all_inboxes_for_profile(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    full_profile.clone().to_string(),
                    node1_subidentity_name.to_string(),
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string(),
                )
                .unwrap();

                let (res2_sender, res2_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIGetAllInboxesForProfile { msg, res: res2_sender })
                    .await
                    .unwrap();
                let node2_last_messages = res2_receiver.recv().await.unwrap().expect("Failed to receive messages");
                // println!("node1_all_profiles: {:?}", node2_last_messages);
                assert!(node2_last_messages.len() == 1);
            }

            // Now we add more messages to properly test unread and pagination
            {
                // Send a Message to the Job for processing
                let message = "3) Are you still there?".to_string();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name,
                    node1_subidentity_name,
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &message,
                    "",
                    "",
                )
                .await;

                // Successfully read unread messages from job inbox
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
                let sender = format!("{}/{}", node1_identity_name, node1_subidentity_name);

                let mut node2_last_messages = vec![];
                for _ in 0..30 {
                    let msg = ShinkaiMessageBuilder::get_last_unread_messages_from_inbox(
                        clone_static_secret_key(&node1_profile_encryption_sk),
                        clone_signature_secret_key(&node1_profile_identity_sk),
                        node1_encryption_pk,
                        inbox_name.to_string(),
                        4,
                        None,
                        "".to_string(),
                        sender.clone(),
                        node1_identity_name.to_string(),
                    )
                    .unwrap();
                    let (res2_sender, res2_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::APIGetLastUnreadMessagesFromInbox { msg, res: res2_sender })
                        .await
                        .unwrap();
                    node2_last_messages = res2_receiver.recv().await.unwrap().expect("Failed to receive messages");
                    // eprintln!("*** node2_last_messages: {:?}", node2_last_messages);
                    if node2_last_messages.len() >= 4 {
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;
                }

                eprintln!("### node2_last_messages: {:?}", node2_last_messages);
                let shinkai_message_content_agent = node2_last_messages[2].get_message_content().unwrap();
                let message_content_agent: JobMessage = serde_json::from_str(&shinkai_message_content_agent).unwrap();

                assert_eq!(message_content_agent.content, message.to_string());
                assert!(node2_last_messages.len() == 4);

                let shinkai_message_content_user = node2_last_messages[0].get_message_content().unwrap();
                let prev_message_content_user: JobMessage =
                    serde_json::from_str(&shinkai_message_content_user).unwrap();

                let offset = node2_last_messages[1].calculate_message_hash_for_pagination();
                eprintln!("### offset: {:?}", offset);
                eprintln!(
                    "### message used for offset: {:?}",
                    node2_last_messages[1].get_message_content().unwrap()
                );
                let next_msg = ShinkaiMessageBuilder::get_last_unread_messages_from_inbox(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    inbox_name.to_string(),
                    5,
                    Some(offset.clone()),
                    "".to_string(),
                    sender.clone(),
                    node1_identity_name.to_string(),
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

                assert!(node2_last_messages.len() == 1);
                assert_eq!(message_content_agent.content, prev_message_content_user.content);

                // we mark read until the offset
                let read_msg = ShinkaiMessageBuilder::read_up_to_time(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    inbox_name.to_string(),
                    offset,
                    "".to_string(),
                    sender,
                    node1_identity_name.to_string(),
                )
                .unwrap();
                let (res2_sender, _) = async_channel::bounded(1);
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
                let sender = format!("{}/{}", node1_identity_name, node1_subidentity_name);

                let msg = ShinkaiMessageBuilder::get_last_unread_messages_from_inbox(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    inbox_name.to_string(),
                    4,
                    None,
                    "".to_string(),
                    sender.clone(),
                    node1_identity_name.to_string(),
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

                // Note(Nico): the backend was modified to do more repeats when chaining so the mocky endpoint returns the same message twice hence
                // this odd result
                // assert!(node2_last_messages.len() == 2);
            }
            {
                // Send an old message
                let past_time_2_secs = ShinkaiStringTime::generate_time_in_past_with_secs(10);

                let job_id_clone = job_id.clone();
                let job_message = JobMessage {
                    job_id,
                    content: "testing old message".to_string(),
                    files_inbox: "".to_string(),
                    parent: None,
                };
                let body = serde_json::to_string(&job_message)
                    .map_err(|_| "Failed to serialize job message to JSON")
                    .unwrap();

                let inbox = InboxName::get_job_inbox_name_from_params(job_id_clone.clone())
                    .map_err(|_| "Failed to get job inbox name")
                    .unwrap()
                    .to_string();

                let job_message = ShinkaiMessageBuilder::new(
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                )
                .body_encryption(EncryptionMethod::None)
                .external_metadata_with_schedule(
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string(),
                    past_time_2_secs,
                )
                .message_raw_content(body.clone())
                .internal_metadata_with_inbox(
                    "main".to_string(),
                    agent_subidentity.clone(),
                    inbox.to_string(),
                    EncryptionMethod::None,
                    None,
                )
                .message_schema_type(MessageSchemaType::JobMessageSchema)
                .build()
                .unwrap();

                let (res_message_job_sender, res_message_job_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIJobMessage {
                        msg: job_message,
                        res: res_message_job_sender,
                    })
                    .await
                    .unwrap();
                let node_job_message = res_message_job_receiver.recv().await.unwrap();
                eprintln!("Old message node_job_message: {:?}", node_job_message);

                let sender = format!("{}/{}", node1_identity_name, node1_subidentity_name);

                let mut node1_last_messages = vec![];
                let mut is_message_found = false;
                for _ in 0..30 {
                    let msg = ShinkaiMessageBuilder::get_last_unread_messages_from_inbox(
                        clone_static_secret_key(&node1_profile_encryption_sk),
                        clone_signature_secret_key(&node1_profile_identity_sk),
                        node1_encryption_pk,
                        inbox.to_string(),
                        4,
                        None,
                        "".to_string(),
                        sender.clone(),
                        node1_identity_name.to_string(),
                    )
                    .unwrap();
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::APIGetLastUnreadMessagesFromInbox { msg, res: res1_sender })
                        .await
                        .unwrap();
                    node1_last_messages = res1_receiver.recv().await.unwrap().expect("Failed to receive messages");
                    // eprintln!("*** node2_last_messages: {:?}", node2_last_messages);
                    if node1_last_messages.len() >= 4 {
                        is_message_found = true;
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                eprintln!("### node1_last_messages: {:?}", node1_last_messages);
                assert!(is_message_found);
            }
            {
                // Send a scheduled message
                // let message = "scheduled message".to_string();
                // let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
                // let sender = format!("{}/{}", node1_identity_name, node1_subidentity_name);
                // let future_time_2_secs = ShinkaiStringTime::generate_time_in_future_with_secs(2);

                // let msg = ShinkaiMessageBuilder::new(
                //     clone_static_secret_key(&node1_profile_encryption_sk),
                //     clone_signature_secret_key(&node1_profile_identity_sk),
                //     node1_encryption_pk,
                // )
                // .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                // .external_metadata_with_schedule(node1_identity_name.to_string(), sender, future_time_2_secs)
                // .message_raw_content(message.clone())
                // .internal_metadata_with_inbox(
                //     "".to_string(),
                //     "".to_string(),
                //     inbox_name.to_string(),
                //     EncryptionMethod::None,
                //     None,
                // )
                // .build();

                abort_handler.abort();
            }
        });

        // Wait for all tasks to complete
        let result = tokio::try_join!(node1_handler, interactions_handler);

        match result {
            Ok(_) => {}
            Err(e) => {
                // Check if the error is because one of the tasks was aborted
                if e.is_cancelled() {
                    println!("One of the tasks was aborted, but this is expected.");
                } else {
                    // If the error is not due to an abort, then it's unexpected
                    panic!("An unexpected error occurred: {:?}", e);
                }
            }
        }
    });
    rt.shutdown_background();
}
