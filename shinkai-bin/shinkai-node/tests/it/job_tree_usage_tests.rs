use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, GenericAPI, Ollama, OpenAI, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
    unsafe_deterministic_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_node::db::db_cron_task::CronTask;
use shinkai_node::network::node_commands::NodeCommand;
use shinkai_node::planner::kai_files::{KaiJobFile, KaiSchemaType};
use std::env;
use std::time::Duration;
use std::time::Instant;
use utils::test_boilerplate::run_test_one_node_network;

use super::utils;
use super::utils::node_test_api::{
    api_agent_registration, api_create_job, api_initial_registration_with_no_code_for_device, api_message_job,
};
use mockito::Server;

#[test]
#[ignore]
fn job_tree_usage_tests() {
     
    let mut server = Server::new();
    
    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_agent = env.node1_agent.clone();
            let node1_encryption_pk = env.node1_encryption_pk.clone();
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);

            // For this test
            let symmetrical_sk = unsafe_deterministic_aes_encryption_key(0);

            {
                // Register a Profile in Node1 and verifies it
                eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
                api_initial_registration_with_no_code_for_device(
                    node1_commands_sender.clone(),
                    env.node1_profile_name.as_str(),
                    env.node1_identity_name.as_str(),
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_device_identity_sk),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_device_name.as_str(),
                )
                .await;
            }
            
            {
                // Register an Agent
                eprintln!("\n\nRegister an Agent in Node1 and verify it");
                let agent_name = ShinkaiName::new(
                    format!(
                        "{}/{}/agent/{}",
                        node1_identity_name.clone(),
                        node1_profile_name.clone(),
                        node1_agent.clone()
                    )
                    .to_string(),
                )
                .unwrap();

                // Note: this is mocked for Ollamas API
                let _m = server
                    .mock("POST", "/api/generate")
                    .with_status(200)
                    .with_header("content-type", "application/json")
                    .with_body(
                        r#"{
                            "model":"mixtral:8x7b-instruct-v0.1-q4_1",
                            "created_at":"2023-12-19T11:36:44.687874415Z",
                            "response":"{\"answer\": \"Why couldn't the bicycle stand up by itself? Because it was two-tired.\"}",
                            "done":true,
                            "context":[28705,733,16289,28793,28705,995,460,396,10023,13892,693,865,659,2735,298,272,3857,3036,304,574,1216,4788,298,4372,707,2996,272,2188,5312,28723,2378,459,1460,354,3629,2758,442,1871,297,574,4372,298,272,2188,28725,562,3768,1912,272,2188,390,1188,1871,390,2572,28723,415,2188,659,2261,28747,28705,387,1912,528,264,13015,28723,13,1047,368,506,2066,1871,298,5090,4372,272,2188,28742,28713,2996,28747,1992,19571,1413,272,2296,413,6848,28765,304,7771,2511,1112,28747,464,18437,464,24115,28742,464,14243,1423,464,11339,28705,1047,368,927,298,18896,680,1871,298,9222,4372,272,2188,28725,868,368,622,927,298,1073,9547,304,1605,27674,4916,28748,14104,272,6594,14060,395,680,1871,304,1073,302,264,3472,5709,298,1300,633,3036,28723,11147,354,28049,680,4842,567,10537,821,1552,28707,479,528,264,28832,28747,1992,19571,1413,272,2296,413,6848,28765,304,7771,2511,1112,28747,464,18437,464,2360,28742,464,14243,1423,28725,464,3499,1869,464,1427,28742,443,28742,28705,8789,3371,733,28748,16289,4490,28739,24115,1264,345,7638,3481,28742,28707,272,24521,1812,1876,582,486,3837,28804,5518,378,403,989,28733,28707,1360,611,28752],
                            "total_duration":29617027653,
                            "load_duration":7157879293,
                            "prompt_eval_count":203,
                            "prompt_eval_duration":19022360000,
                            "eval_count":25,
                            "eval_duration":3435284000
                        }"#,
                    )
                    .create();

                let open_ai = OpenAI {
                    model_type: "gpt-4-vision-preview".to_string(),
                };

                let ollama = Ollama {
                    model_type: "mixtral:8x7b-instruct-v0.1-q4_1".to_string(),
                };

                let api_key = env::var("INITIAL_AGENT_API_KEY").expect("API_KEY must be set");

                let agent = SerializedLLMProvider {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    perform_locally: false,
                    // external_url: Some("http://localhost:11435".to_string()),
                    // external_url: Some("https://api.openai.com".to_string()),
                    api_key: Some("".to_string()),
                    // api_key: Some(api_key),
                    external_url: Some(server.url()),
                    // api_key: Some("mockapikey".to_string()),
                    // external_url: Some("https://api.together.xyz".to_string()),
                    // model: LLMProviderInterface::OpenAI(open_ai),
                    // model: LLMProviderInterface::GenericAPI(generic_api),
                    model: LLMProviderInterface::Ollama(ollama),
                    toolkit_permissions: vec![],
                    storage_bucket_permissions: vec![],
                    allowed_message_senders: vec![],
                };
                api_agent_registration(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    agent,
                )
                .await;
            }

            let mut job_id = "".to_string();
            let agent_subidentity = format!("{}/agent/{}", node1_profile_name.clone(), node1_agent.clone()).to_string();
            {
                // Create a Job
                eprintln!("\n\nCreate a Job for the previous Agent in Node1 and verify it");
                job_id = api_create_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                )
                .await;
            }
            let job_message_content = "tell me a joke".to_string();
            {
                // Send a Message to the Job for processing
                eprintln!("\n\nSend a message for the Job");
                let start = Instant::now();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &job_message_content,
                    "",
                )
                .await;

                let duration = start.elapsed(); // Get the time elapsed since the start of the timer
                eprintln!("Time elapsed in api_message_job is: {:?}", duration);
            }
            {
                eprintln!("Waiting for the Job to finish");
                tokio::time::sleep(Duration::from_secs(2)).await;
                let mut job_completed = false;
                for _ in 0..5 {
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::FetchLastMessages {
                            limit: 4,
                            res: res1_sender,
                        })
                        .await
                        .unwrap();
                    let node1_last_messages = res1_receiver.recv().await.unwrap();
                    eprintln!("node1_last_messages: {:?}", node1_last_messages);

                    if node1_last_messages.len() >= 2 {
                        match node1_last_messages[1].get_message_content() {
                            Ok(message_content) => match serde_json::from_str::<JobMessage>(&message_content) {
                                Ok(job_message) => {
                                    eprintln!("message_content: {}", job_message.content);
                                    job_completed = true;
                                    break;
                                }
                                Err(_) => {
                                    eprintln!("error: message_content: {}", message_content);
                                }
                            },
                            Err(_) => {
                                // nothing
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                assert!(job_completed, "Job did not complete within the expected time");
            }
            let second_job_message_content = "I didn't understand the joke. Can you explain it?".to_string();
            {
                // Sending a second message to the Job for processing
                eprintln!("\n\nSend a second message for the Job");
                let start = Instant::now();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &second_job_message_content,
                    "",
                )
                .await;

                let duration = start.elapsed(); // Get the time elapsed since the start of the timer
                eprintln!("Time elapsed in api_message_job is: {:?}", duration);
            }
            {
                eprintln!("Waiting for the Job to finish");
                tokio::time::sleep(Duration::from_secs(2)).await;
                let mut job_completed = false;
                for _ in 0..5 {
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::FetchLastMessages {
                            limit: 4,
                            res: res1_sender,
                        })
                        .await
                        .unwrap();
                    let node1_last_messages = res1_receiver.recv().await.unwrap();
                    eprintln!("node1_last_messages: {:?}", node1_last_messages);

                    if node1_last_messages.len() >= 4 {
                        match node1_last_messages[3].get_message_content() {
                            Ok(message_content) => match serde_json::from_str::<JobMessage>(&message_content) {
                                Ok(job_message) => {
                                    eprintln!("message_content: {}", job_message.content);
                                    job_completed = true;
                                    break;
                                }
                                Err(_) => {
                                    eprintln!("error: message_content: {}", message_content);
                                }
                            },
                            Err(_) => {
                                // nothing
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                assert!(job_completed, "Job did not complete within the expected time");
            }
        })
    });
}
