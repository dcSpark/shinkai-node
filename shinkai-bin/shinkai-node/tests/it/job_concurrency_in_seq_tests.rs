// use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{LLMProviderInterface, OpenAI, SerializedLLMProvider};
// use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
// use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
// use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
// use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
// use shinkai_node::network::node::NodeCommand;
// use std::time::Duration;
// use std::time::Instant;
// use utils::test_boilerplate::run_test_one_node_network;

// use super::utils;
// use super::utils::node_test_api::{
//     api_agent_registration, api_create_job, api_initial_registration_with_no_code_for_device, api_message_job,
// };
// use mockito::Server;

// TODO: will enable back again
// #[test]
// fn job_concurrency_in_seq() {
//     init_default_tracing();
//     run_test_one_node_network(|env| {
//         Box::pin(async move {
//             let node1_commands_sender = env.node1_commands_sender.clone();
//             let node1_identity_name = env.node1_identity_name.clone();
//             let node1_profile_name = env.node1_profile_name.clone();
//             let node1_device_name = env.node1_device_name.clone();
//             let node1_agent = env.node1_agent.clone();
//             let node1_encryption_pk = env.node1_encryption_pk;
//             let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
//             let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
//             let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
//             let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
//             let node1_abort_handler = env.node1_abort_handler;

//             // For this test
//             {
//                 // Register a Profile in Node1 and verifies it
//                 eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
//                 api_initial_registration_with_no_code_for_device(
//                     node1_commands_sender.clone(),
//                     env.node1_profile_name.as_str(),
//                     env.node1_identity_name.as_str(),
//                     node1_encryption_pk,
//                     node1_device_encryption_sk.clone(),
//                     clone_signature_secret_key(&node1_device_identity_sk),
//                     node1_profile_encryption_sk.clone(),
//                     clone_signature_secret_key(&node1_profile_identity_sk),
//                     node1_device_name.as_str(),
//                 )
//                 .await;
//             }
//             let mut server = Server::new();
//             {
//                 // Register an Agent
//                 eprintln!("\n\nRegister an Agent in Node1 and verify it");
//                 let agent_name = ShinkaiName::new(
//                     format!(
//                         "{}/{}/agent/{}",
//                         node1_identity_name.clone(),
//                         node1_profile_name.clone(),
//                         node1_agent.clone()
//                     )
//                     .to_string(),
//                 )
//                 .unwrap();

//                 let _m = server
//                     .mock("POST", "/v1/chat/completions")
//                     .match_header("authorization", "Bearer mockapikey")
//                     .with_status(200)
//                     .with_header("content-type", "application/json")
//                     .with_body(
//                         r#"{
//                     "id": "chatcmpl-123",
//                     "object": "chat.completion",
//                     "created": 1677652288,
//                     "choices": [{
//                         "index": 0,
//                         "message": {
//                             "role": "assistant",
//                             "content": "\n\n{\"answer\": \"Hello there, how may I assist you today?\"}"
//                         },
//                         "finish_reason": "stop"
//                     }],
//                     "usage": {
//                         "prompt_tokens": 9,
//                         "completion_tokens": 12,
//                         "total_tokens": 21
//                     }
//                 }"#,
//                     )
//                     .create();

//                 let open_ai = OpenAI {
//                     model_type: "gpt-4-1106-preview".to_string(),
//                 };

//                 let agent = SerializedLLMProvider {
//                     id: node1_agent.clone().to_string(),
//                     full_identity_name: agent_name,
//                     perform_locally: false,
//                     external_url: Some(server.url()),
//                     api_key: Some("mockapikey".to_string()),
//                     model: LLMProviderInterface::OpenAI(open_ai),
//                     toolkit_permissions: vec![],
//                     storage_bucket_permissions: vec![],
//                     allowed_message_senders: vec![],
//                 };
//                 api_agent_registration(
//                     node1_commands_sender.clone(),
//                     clone_static_secret_key(&node1_profile_encryption_sk),
//                     node1_encryption_pk.clone(),
//                     clone_signature_secret_key(&node1_profile_identity_sk),
//                     node1_identity_name.clone().as_str(),
//                     node1_profile_name.clone().as_str(),
//                     agent,
//                 )
//                 .await;
//             }

//             let mut job_id = "".to_string();
//             let agent_subidentity = format!("{}/agent/{}", node1_profile_name.clone(), node1_agent.clone()).to_string();
//             {
//                 // Create a Job
//                 eprintln!("\n\nCreate a Job for the previous Agent in Node1 and verify it");
//                 job_id = api_create_job(
//                     node1_commands_sender.clone(),
//                     clone_static_secret_key(&node1_profile_encryption_sk),
//                     node1_encryption_pk.clone(),
//                     clone_signature_secret_key(&node1_profile_identity_sk),
//                     node1_identity_name.clone().as_str(),
//                     node1_profile_name.clone().as_str(),
//                     &agent_subidentity.clone(),
//                 )
//                 .await;
//             }
//             let job_message_content = "hello are u there?".to_string();
//             {
//                 // Send a Message to the Job for processing multiple times
//                 eprintln!("\n\nSend a message to the Job multiple times");
//                 let start = Instant::now();

//                 for _ in 0..4 {
//                     api_message_job(
//                         node1_commands_sender.clone(),
//                         clone_static_secret_key(&node1_profile_encryption_sk),
//                         node1_encryption_pk.clone(),
//                         clone_signature_secret_key(&node1_profile_identity_sk),
//                         node1_identity_name.clone().as_str(),
//                         node1_profile_name.clone().as_str(),
//                         &agent_subidentity.clone(),
//                         &job_id.clone().to_string(),
//                         &job_message_content,
//                         "",
//                         "",
//                     )
//                     .await;
//                 }

//                 let duration = start.elapsed(); // Get the time elapsed since the start of the first message
//                 eprintln!("Time elapsed in sending messages is: {:?}", duration);
//             }
//             {
//                 eprintln!("Waiting for the Job to finish");
//                 let start = Instant::now();
//                 loop {
//                     let (res1_sender, res1_receiver) = async_channel::bounded(1);
//                     node1_commands_sender
//                         .send(NodeCommand::FetchLastMessages {
//                             limit: 8, // Set the limit to 8 to fetch up to 8 messages
//                             res: res1_sender,
//                         })
//                         .await
//                         .unwrap();
//                     let node1_last_messages = res1_receiver.recv().await.unwrap();
//                     // eprintln!("node1_last_messages: {:?}", node1_last_messages);
//                     eprintln!("### node1_last_messages.len(): {:?}", node1_last_messages.len());

//                     if node1_last_messages.len() >= 8 {
//                         break; // Break the loop if we have 12 or more messages
//                     }

//                     if start.elapsed() > Duration::from_secs(60) {
//                         panic!("Test failed: 5 seconds have passed without receiving 12 messages");
//                     }

//                     tokio::time::sleep(Duration::from_millis(500)).await; // Short sleep to prevent tight looping
//                 }
//             }
//             node1_abort_handler.abort();
//         })
//     });
// }
