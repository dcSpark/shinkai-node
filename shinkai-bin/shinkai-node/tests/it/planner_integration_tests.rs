use super::utils::test_boilerplate::run_test_one_node_network;
use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use mockito::{Matcher, Mock};
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, OpenAI, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{JobMessage, MessageSchemaType};
use shinkai_message_primitives::shinkai_utils::encryption::{clone_static_secret_key, EncryptionMethod};
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
    unsafe_deterministic_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_node::llm_provider::error::LLMProviderError;
use shinkai_node::cron_tasks::web_scrapper::CronTaskRequest;
use shinkai_node::db::db_cron_task::CronTask;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::planner::kai_files::{KaiJobFile, KaiSchemaType};
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use std::time::Instant;

use super::utils::node_test_api::{
    api_agent_registration, api_create_job, api_get_all_inboxes_from_profile, api_get_all_smart_inboxes_from_profile,
    api_initial_registration_with_no_code_for_device, api_message_job,
};
use mockito::Server;

fn create_mock_openai(server: &mut mockito::Server, request_body: &str, response_body: &str) -> Mock {
    // Parse the response_body into a JSON Value
    let response_json: serde_json::Value =
        serde_json::from_str(response_body).expect("Invalid JSON string provided for response_body");

    // Extract the content field
    let content = response_json["choices"][0]["message"]["content"]
        .as_str()
        .expect("Failed to extract content field from response_body");

    // Validate that content is a valid JSON object
    match extract_largest_json_object(content) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to extract JSON object from content: {}", content);
            panic!("Failed to extract JSON object from content: {}", e);
        }
    }

    let m = server
        .mock("POST", "/v1/chat/completions")
        .match_header("authorization", "Bearer mockapikey")
        .match_body(Matcher::JsonString(request_body.to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(response_body)
        .create();
    m
}

#[test]
#[ignore]
fn planner_integration_test() {
    init_default_tracing();
    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_agent = env.node1_agent.clone();
            let node1_encryption_pk = env.node1_encryption_pk;
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);

            // For this test
            let symmetrical_sk = unsafe_deterministic_aes_encryption_key(0);
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
            let mut server = Server::new();
            {
                // Register an Agent
                eprintln!("\n\nRegister an Agent in Node1 and verify it");
                let _ = create_openai_mock_1(&mut server);
                let _ = create_openai_mock_2(&mut server);
                let _ = create_openai_mock_3(&mut server);

                let open_ai = OpenAI {
                    model_type: "gpt-4-1106-preview".to_string(),
                    // model_type: "gpt-3.5-turbo-1106".to_string(),
                };

                // let generic_api = GenericAPI {
                //     model_type: "togethercomputer/llama-2-70b-chat".to_string(),
                // };

                let api_key = env::var("INITIAL_AGENT_API_KEY").expect("API_KEY must be set");

                let agent = SerializedLLMProvider {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name.clone(),
                    perform_locally: false,
                    // external_url: Some(server.url()),
                    // api_key: Some("mockapikey".to_string()),
                    external_url: Some("https://api.openai.com".to_string()),
                    api_key: Some(api_key),
                    // external_url: Some("https://api.together.xyz".to_string()),
                    model: LLMProviderInterface::OpenAI(open_ai),
                    // model: LLMProviderInterface::GenericAPI(generic_api),
                    toolkit_permissions: vec![],
                    storage_bucket_permissions: vec![],
                    allowed_message_senders: vec![],
                };
                api_agent_registration(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
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
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                )
                .await;
            }
            {
                // api_get_all_inboxes_from_profile
                eprintln!("\n\nGet All Profiles");
                let inboxes = api_get_all_inboxes_from_profile(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    node1_identity_name.clone().as_str(),
                )
                .await;
                assert_eq!(inboxes.len(), 1);
            }
            // Send message (APICreateFilesInboxWithSymmetricKey) from Device subidentity to Node 1
            {
                eprintln!("\n\n### Sending message (APICreateFilesInboxWithSymmetricKey) from profile subidentity to node 1\n\n");

                let message_content = aes_encryption_key_to_string(symmetrical_sk);
                let msg = ShinkaiMessageBuilder::create_files_inbox_with_sym_key(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    "job::test::false".to_string(),
                    message_content.clone(),
                    node1_profile_name.to_string(),
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string(),
                )
                .unwrap();

                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APICreateFilesInboxWithSymmetricKey { msg, res: res_sender })
                    .await
                    .unwrap();
                let _response = res_receiver.recv().await.unwrap().expect("Failed to receive messages");
            }
            {
                eprintln!("\n\n### Sending Message (APIAddFileToInboxWithSymmetricKey) from profile subidentity to node 1\n\n");

                let cron_request = CronTaskRequest {
                    crawl_links: false,
                    cron_description: "Every day at 8pm".to_string(),
                    task_description: "Find all the news related to AI in a website".to_string(),
                    object_description: Some("https://news.ycombinator.com".to_string()),
                };

                // Create a KaiJobFile from the CronTaskRequest
                let kai_file = KaiJobFile {
                    schema: KaiSchemaType::CronJobRequest(cron_request),
                    shinkai_profile: None,
                    agent_id: node1_agent.clone().to_string(),
                };

                // Serialize the KaiJobFile to a JSON string
                let json_string = kai_file.to_json_str().unwrap();

                // Convert the JSON string to a Vec<u8>
                let file_data: Vec<u8> = json_string.into_bytes();

                // Encrypt the file using Aes256Gcm
                let cipher = Aes256Gcm::new(GenericArray::from_slice(&symmetrical_sk));
                let nonce = GenericArray::from_slice(&[0u8; 12]);
                let nonce_slice = nonce.as_slice();
                let nonce_str = aes_nonce_to_hex_string(nonce_slice);
                let ciphertext = cipher.encrypt(nonce, file_data.as_ref()).expect("encryption failure!");

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIAddFileToInboxWithSymmetricKey {
                        filename: "cron_request.jobkai".to_string(),
                        file: ciphertext,
                        public_key: hash_of_aes_encryption_key_hex(symmetrical_sk),
                        encrypted_nonce: nonce_str,
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                // Receive the response
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("response: {}", response);
            }
            {
                // Get filenames in inbox
                let message_content = hash_of_aes_encryption_key_hex(symmetrical_sk);
                let msg = ShinkaiMessageBuilder::new(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                )
                .message_raw_content(message_content.clone())
                .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                .message_schema_type(MessageSchemaType::TextContent)
                .internal_metadata(
                    node1_profile_name.to_string().clone(),
                    "".to_string(),
                    EncryptionMethod::None,
                    None,
                )
                .external_metadata_with_intra_sender(
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string().clone(),
                    node1_profile_name.to_string().clone(),
                )
                .build()
                .unwrap();

                let (res_sender, res_receiver) = async_channel::bounded(1);
                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIGetFilenamesInInbox { msg, res: res_sender })
                    .await
                    .unwrap();

                // Receive the response
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("response: {:?}", response);
            }
            let job_message_content = "".to_string();
            {
                // Send a Message to the Job for processing
                eprintln!("\n\nSend a message for the Job");
                let start = Instant::now();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &job_message_content,
                    &hash_of_aes_encryption_key_hex(symmetrical_sk),
                    "",
                    None,
                )
                .await;

                let duration = start.elapsed(); // Get the time elapsed since the start of the timer
                eprintln!("Time elapsed in api_message_job is: {:?}", duration);
            }
            {
                // api_get_all_smart_inboxes_from_profile
                eprintln!("\n\n Get All Smart Inboxes");
                let inboxes = api_get_all_smart_inboxes_from_profile(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    node1_identity_name.clone().as_str(),
                )
                .await;
                assert_eq!(inboxes.len(), 1);
                eprintln!("inboxes: {:?}", inboxes);
            }
            {
                eprintln!("Waiting for the Job to finish");
                let mut job_finished = false;
                for _ in 0..10 {
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::FetchLastMessages {
                            limit: 2,
                            res: res1_sender,
                        })
                        .await
                        .unwrap();
                    let node1_last_messages = res1_receiver.recv().await.unwrap();
                    eprintln!("node1_last_messages: {:?}", node1_last_messages);

                    match node1_last_messages[0].get_message_content() {
                        Ok(message_content) => match serde_json::from_str::<JobMessage>(&message_content) {
                            Ok(job_message) => {
                                if job_message.content != job_message_content {
                                    eprintln!("job_message.content: {}", job_message.content);
                                    job_finished = true;
                                    break;
                                }
                            }
                            Err(_) => {
                                eprintln!("error: message_content: {}", message_content);
                            }
                        },
                        Err(_) => {
                            // nothing
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(20)).await;
                }
                if !job_finished {
                    eprintln!("Job didn't finish in time");
                    panic!("Job didn't finish in time");
                }
            }
            {
                // Send a dummy Message to test that the algorithm can still find the kaijob file
                eprintln!("\n\nSend a message for the Job");
                let start = Instant::now();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    "dummy message",
                    "",
                    "",
                    None,
                )
                .await;

                let duration = start.elapsed(); // Get the time elapsed since the start of the timer
                eprintln!("Time elapsed in api_message_job is: {:?}", duration);
            }
            {
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone().to_string()).unwrap();

                // Create a ShinkaiMessage for the command
                let msg = ShinkaiMessageBuilder::new(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                )
                .message_raw_content(job_id.clone())
                .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                .message_schema_type(MessageSchemaType::APIFinishJob)
                .internal_metadata_with_inbox(
                    node1_profile_name.to_string().clone(),
                    "".to_string(),
                    inbox_name.to_string(),
                    EncryptionMethod::None,
                    None,
                )
                .external_metadata_with_intra_sender(
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string().clone(),
                    node1_profile_name.to_string().clone(),
                )
                .build()
                .unwrap();

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIUpdateJobToFinished { msg, res: res_sender })
                    .await
                    .unwrap();

                // Receive the response
                res_receiver.recv().await.unwrap().expect("Failed to receive response");
                tokio::time::sleep(Duration::from_secs(360)).await;
            }
            // Test APIPrivateDevopsCronList
            {
                eprintln!("Testing APIPrivateDevopsCronList");
                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIPrivateDevopsCronList { res: res_sender })
                    .await
                    .unwrap();
                let response = res_receiver.recv().await.unwrap();
                eprintln!("response: {:?}", response);
                match response {
                    Ok(tasks_json) => {
                        let tasks_map: HashMap<String, CronTask> = serde_json::from_str(&tasks_json).unwrap();
                        let tasks: Vec<CronTask> = tasks_map.into_iter().map(|(_, task)| task).collect();
                        assert!(!tasks.is_empty(), "No cron tasks were returned");
                    }
                    Err(err) => {
                        panic!("APIPrivateDevopsCronList returned an error: {}", err.message);
                    }
                }
            }
        })
    });
}

fn create_openai_mock_1(server: &mut mockito::Server) -> Mock {
    create_mock_openai(
        server,
        r#"{
            "max_tokens": 4096,
            "messages": [
                {
                    "content": "You are a very helpful assistant that's an expert in translating user requests to cron expressions.",
                    "role": "system"
                },
                {
                    "content": "The current task at hand is create a cron expression using the following description:\n\n`Every day at 8pm`",
                    "role": "user"
                },
                {
                    "content": "Respond using the following markdown formatting and absolutely nothing else: # Answer",
                    "role": "system"
                }
            ],
            "model": "gpt-4-1106-preview",
            "temperature": 0.7
        }"#,
        r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "\n\n{\"answer\": \"0 20 * * *\"}"
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
}

// Note: it's important to escape slashes inside message.content.answer
fn create_openai_mock_2(server: &mut mockito::Server) -> Mock {
    create_mock_openai(
        server,
        r#"{
            "max_tokens": 4096,
            "messages": [
                {
                    "content": "You are an autoregressive language model that has been fine-tuned with instruction-tuning and RLHF. You carefully provide accurate, factual, thoughtful, nuanced answers, and are brilliant at reasoning. If you think there might not be a correct answer, you say so.  Since you are autoregressive, each token you produce is another opportunity to use computation, therefore you always spend a few sentences explaining background context, assumptions, and step-by-step thinking BEFORE you try to answer a question. You are a very helpful assistant with PDDL planning expertise and access to a series of tools. The only tools at your disposal for PDDL planing are: ---tools--- Toolkit Name: web_link_extractor\nDescription: Extracts all hyperlinks from the provided HTML string.\nInput Name: html\nInput EBNF: html :== \"(.*)\"\nOutput Name: links\nOutput EBNF: links :== \\[\"(.*)\"\\]\n\n\nToolkit Name: html_extractor\nDescription: Fetches HTML content from the specified URL.\nInput Name: url\nInput EBNF: url :== \"http(s)?://([\\w-]+\\.)+[\\w-]+(/[\\w- ./?%&=]*)?\"\nOutput Name: htmlContent\nOutput EBNF: htmlContent :== \"(.*)\"\n\n\nToolkit Name: content_summarizer\nDescription: Generates a concise summary of the provided text content. It could be a website.\nInput Name: text\nInput EBNF: text :== \"(.*)\"\nInput Name: summaryLength\nInput EBNF: summaryLength :== ([0-9]+)\nOutput Name: summary\nOutput EBNF: summary :== \"(.*)\"\n\n\nToolkit Name: LLM_caller\nDescription: Ask an LLM any questions (it won't know current information).\nInput Name: prompt\nInput EBNF: prompt :== \"(.*)\"\nOutput Name: response\nOutput EBNF: response :== \"(.*)\"\n ---end_tools---",
                    "role": "system"
                },
                {
                    "content": "You always remember that a PDDL is formatted like this (unrelated example): --start example---(define (domain letseat)\n    (:requirements :typing)\n\n    (:types\n        location locatable - object\n        bot cupcake - locatable\n        robot - bot\n    )\n\n    (:predicates\n        (on ?obj - locatable ?loc - location)\n        (holding ?arm - locatable ?cupcake - locatable)\n        (arm-empty)\n        (path ?location1 - location ?location2 - location)\n    )\n\n    (:action pick-up\n        :parameters (?arm - bot ?cupcake - locatable ?loc - location)\n        :precondition (and\n            (on ?arm ?loc)\n            (on ?cupcake ?loc)\n            (arm-empty)\n        )\n        :effect (and\n            (not (on ?cupcake ?loc))\n            (holding ?arm ?cupcake)\n            (not (arm-empty))\n        )\n    )\n\n    (:action drop\n        :parameters (?arm - bot ?cupcake - locatable ?loc - location)\n        :precondition (and\n            (on ?arm ?loc)\n            (holding ?arm ?cupcake)\n        )\n        :effect (and\n            (on ?cupcake ?loc)\n            (arm-empty)\n            (not (holding ?arm ?cupcake))\n        )\n    )\n\n    (:action move\n        :parameters (?arm - bot ?from - location ?to - location)\n        :precondition (and\n            (on ?arm ?from)\n            (path ?from ?to)\n        )\n        :effect (and\n            (not (on ?arm ?from))\n            (on ?arm ?to)\n        )\n    )\n)---end example---",
                    "role": "user"
                },
                {
                    "content": "The current task at hand is to: 'Find all the news related to AI in a website'. Implement a throughout plan using PDDL representation using the available tools. (define (domain ",
                    "role": "user"
                },
                {
                    "content": "Take a deep breath and think step by step, explain how to implement this in the explanation field and then put your final answer in the answer field",
                    "role": "user"
                },
                {
                    "content": "Respond using the following EBNF and absolutely nothing else: '{' 'explanation' ':' string, 'answer' ':' string '}'  ```json",
                    "role": "system"
                }
            ],
            "model": "gpt-4-1106-preview",
            "temperature": 0.7
        }"#,
        r#"{
            "id": "chatcmpl-8N4ipmnUHFu8Sx1sVAZSMBPsL2eB9",
            "object": "chat.completion",
            "created": 1700510387,
            "model": "gpt-4-1106-preview",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "{\"explanation\": \"some text there\",\"answer\": \"(define (domain findainews)\\n    (:requirements :strips)\\n\\n    (:types\\n        url\\n        html_content\\n        hyperlink\\n    )\\n\\n    (:predicates\\n        (html_fetched ?url - url)\\n        (links_extracted ?html - html_content)\\n        (link_relevant ?link - hyperlink)\\n        (content_summarized ?link - hyperlink)\\n    )\\n\\n    (:action fetch_html\\n        :parameters (?url - url)\\n        :precondition (not (html_fetched ?url))\\n        :effect (html_fetched ?url)\\n    )\\n\\n    (:action extract_links\\n        :parameters (?html - html_content)\\n        :precondition (html_fetched ?html)\\n        :effect (links_extracted ?html)\\n    )\\n\\n    (:action evaluate_relevance\\n        :parameters (?link - hyperlink)\\n        :precondition (links_extracted ?link)\\n        :effect (link_relevant ?link)\\n    )\\n\\n    (:action summarize_content\\n        :parameters (?link - hyperlink)\\n        :precondition (and (link_relevant ?link) (not (content_summarized ?link)))\\n        :effect (content_summarized ?link)\\n    )\\n)\"}"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 892,
                "completion_tokens": 664,
                "total_tokens": 1556
            },
            "system_fingerprint": "fp_a24b4d720c"
        }"#,
    )
}

fn create_openai_mock_3(server: &mut mockito::Server) -> Mock {
    create_mock_openai(
        server,
        r#"{
            "max_tokens": 4096,
            "messages": [
                {
                    "content": "You are an autoregressive language model that has been fine-tuned with instruction-tuning and RLHF. You carefully provide accurate, factual, thoughtful, nuanced answers, and are brilliant at reasoning. If you think there might not be a correct answer, you say so.  Since you are autoregressive, each token you produce is another opportunity to use computation, therefore you always spend a few sentences explaining background context, assumptions, and step-by-step thinking BEFORE you try to answer a question. You are a very helpful assistant with PDDL planning expertise and access to a series of tools. The only tools at your disposal for PDDL planing are: ---tools--- Toolkit Name: web_link_extractor\nDescription: Extracts all hyperlinks from the provided HTML string.\nInput Name: html\n\n\nToolkit Name: html_extractor\nDescription: Fetches HTML content from the specified URL.\nInput Name: url\n\n\nToolkit Name: content_summarizer\nDescription: Generates a concise summary of the provided text content. It could be a website.\nInput Name: text\nInput Name: summaryLength\n\n\nToolkit Name: LLM_caller\nDescription: Ask an LLM any questions (it won't know current information).\nInput Name: prompt\n ---end_tools---",
                    "role": "system"
                },
                {
                    "content": "You always remember that a PDDL is formatted like this (unrelated example): ---start example---(define (problem letseat-simple)\n    (:domain letseat)\n    (:objects\n        arm - robot\n        cupcake - cupcake\n        table - location\n        plate - location\n    )\n\n    (:init\n        (on arm table)\n        (on cupcake table)\n        (arm-empty)\n        (path table plate)\n    )\n    (:goal\n        (on cupcake plate)\n    )\n)---end example---",
                    "role": "user"
                },
                {
                    "content": "The current task is to: 'Find all the news related to AI in a website'. Implement a plan using PDDL representation using the available tools. Make it simple but effective and start your response with: (define (problem ",
                    "role": "user"
                },
                {
                    "content": "Take a deep breath and think step by step, explain how to implement this in the explanation field and then put your final answer in the answer field",
                    "role": "user"
                },
                {
                    "content": "Respond using the following EBNF and absolutely nothing else: '{' 'explanation' ':' string, 'answer' ':' string '}'  ```json",
                    "role": "system"
                }
            ],
            "model": "gpt-4-1106-preview",
            "temperature": 0.7
        }"#,
        r#"{
            "id": "chatcmpl-8N4jK4l0Mfxw88I4ZXbUm23QaDZ9z",
            "object": "chat.completion",
            "created": 1700510418,
            "model": "gpt-4-1106-preview",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "{\"explanation\": \"more text.\",\"answer\": \"(define (problem find-ai-news)\\n    (:domain web-news-extraction)\\n    (:objects\\n        website - website\\n        ai_news_articles - article\\n        extracted_links - hyperlink\\n        summarized_content - summary\\n    )\\n\\n    (:init\\n        (unprocessed website)\\n        (available web_link_extractor)\\n        (available html_extractor)\\n        (available content_summarizer)\\n    )\\n    (:goal\\n        (and\\n            (links_extracted website extracted_links)\\n            (content_fetched extracted_links)\\n            (content_summarized ai_news_articles summarized_content)\\n            (ai_related summarized_content)\\n        )\\n    )\\n    (:action extract-links\\n        :parameters (website)\\n        :precondition (unprocessed website)\\n        :effect (links_extracted website extracted_links)\\n    )\\n    (:action fetch-html\\n        :parameters (extracted_links)\\n        :precondition (links_extracted website extracted_links)\\n        :effect (content_fetched extracted_links)\\n    )\\n    (:action summarize-content\\n        :parameters (ai_news_articles)\\n        :precondition (content_fetched extracted_links)\\n        :effect (content_summarized ai_news_articles summarized_content)\\n    )\\n)\"}"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 491,
                "completion_tokens": 533,
                "total_tokens": 1024
            },
            "system_fingerprint": "fp_a24b4d720c"
        }"#,
    )
}

fn extract_largest_json_object(s: &str) -> Result<JsonValue, LLMProviderError> {
    let mut depth = 0;
    let mut start = None;

    for (i, c) in s.char_indices() {
        match c {
            '{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let json_str = &s[start.unwrap()..=i];
                    let json_val: JsonValue = serde_json::from_str(json_str)
                        .map_err(|_| LLMProviderError::FailedExtractingJSONObjectFromResponse(s.to_string()))?;
                    return Ok(json_val);
                }
            }
            _ => {}
        }
    }

    Err(LLMProviderError::FailedExtractingJSONObjectFromResponse(s.to_string()))
}
