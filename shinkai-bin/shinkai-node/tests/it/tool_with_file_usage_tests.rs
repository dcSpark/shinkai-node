use async_channel::{bounded, Receiver, Sender};
use serde_json::{json, Map, Value};
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, OpenAI, SerializedLLMProvider
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tools::DynamicToolType;
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, unsafe_deterministic_encryption_keypair
};
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
use shinkai_message_primitives::shinkai_utils::search_mode::VectorSearchMode;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::Node;
use shinkai_tools_primitives::tools::tool_types::{OperatingSystem, RunnerType};
use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::time::Duration;
use tokio::runtime::Runtime;

use crate::it::utils::node_test_api::{api_create_job_with_scope, api_execute_tool};
use crate::it::utils::vecfs_test_utils::{create_folder, upload_file};

use super::utils::db_handlers::setup_node_storage_path;
use super::utils::node_test_api::{api_registration_device_node_profile_main, wait_for_default_tools};
use super::utils::test_boilerplate::{default_embedding_model, supported_embedding_models};

// Import the necessary types for creating a ShinkaiTool
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::python_tools::PythonTool;
use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolWithAssets};
use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
use shinkai_tools_primitives::tools::tool_types::ToolResult;

use mockito::Server;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

#[test]
fn text_file_copy_tool_test() {
    setup_node_storage_path();
    std::env::set_var("WELCOME_MESSAGE", "false");

    setup();
    let rt = Runtime::new().unwrap();

    let mut server = Server::new();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.sep-shinkai";
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

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name));

        let node1_profile_name = "main";
        let api_key_bearer = "my_api_key".to_string();

        // Agent pre-creation
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
                    "content": "The Roman Empire is very interesting"
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

        let agent = SerializedLLMProvider {
            id: node1_agent.to_string(),
            full_identity_name: agent_name,
            external_url: Some(server.url()),
            api_key: Some("mockapikey".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
        };

        // Create node1
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk.clone(),
            None,
            None,
            0,
            node1_commands_receiver,
            node1_db_path,
            "".to_string(),
            None,
            true,
            vec![agent],
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            Some(api_key_bearer.clone()),
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

                // Wait for default tools to be ready
                let tools_ready = wait_for_default_tools(
                    node1_commands_sender.clone(),
                    api_key_bearer.clone(),
                    20, // Wait up to 20 seconds
                )
                .await
                .expect("Failed to check for default tools");
                assert!(tools_ready, "Default tools should be ready within 20 seconds");
            }
            {
                // Check that Rust tools are installed, retry up to 10 times
                let mut retry_count = 0;
                let max_retries = 40;
                let retry_delay = Duration::from_millis(500);

                loop {
                    tokio::time::sleep(retry_delay).await;

                    let (res_sender, res_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::InternalCheckRustToolsInstallation { res: res_sender })
                        .await
                        .unwrap();

                    match res_receiver.recv().await {
                        Ok(result) => {
                            match result {
                                Ok(has_tools) => {
                                    if has_tools {
                                        // Rust tools are installed, we can break the loop
                                        break;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error checking Rust tools installation: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error receiving check result: {:?}", e);
                            panic!("Error receiving check result: {:?}", e);
                        }
                    }

                    retry_count += 1;
                    if retry_count >= max_retries {
                        panic!("Rust tools were not installed after {} retries", max_retries);
                    }
                }
                eprintln!("Rust tools were installed after {} retries", retry_count);
            }
            {
                // Create a folder and upload a test file
                eprintln!("\n\n### Creating a folder and uploading a test file \n\n");

                // Create test folder
                create_folder(
                    &node1_commands_sender,
                    "/",
                    "test_folder",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;

                // Create a test text file
                let test_file_path = Path::new("test_text_file.txt");
                let test_content = "This is a test file content";
                let mut file = fs::File::create(test_file_path).unwrap();
                file.write_all(test_content.as_bytes()).unwrap();

                // Upload the test file to /test_folder
                upload_file(
                    &node1_commands_sender,
                    "/test_folder",
                    test_file_path,
                    &api_key_bearer.clone(),
                )
                .await;

                // Register the text-file-copy tool
                let python_code = r#"
from typing import Dict, Any, Optional, List
import os
import logging
from shinkai_local_support import get_home_path

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class CONFIG:
    pass

class INPUTS:
    file_path: str

class OUTPUT:
    content: str
    original_file_path: str

async def run(config: CONFIG, inputs: INPUTS) -> OUTPUT:
    logger.info(f"Reading file from path: {inputs.file_path}")
    
    try:
        # Check if file exists
        if not os.path.exists(inputs.file_path):
            logger.error(f"File not found: {inputs.file_path}")
            raise FileNotFoundError(f"File not found: {inputs.file_path}")
        
        # Read file content
        with open(inputs.file_path, 'r', encoding='utf-8') as file:
            content = file.read()
            logger.info(f"Successfully read file with {len(content)} characters")
        
        # Append " copy" to the content
        modified_content = content + " copy"
        logger.info("Added ' copy' to the content")
        
        # Create output
        output = OUTPUT()
        output.content = modified_content
        output.original_file_path = inputs.file_path
        logger.info("File processing completed successfully")
        
        return output
        
    except Exception as e:
        logger.error(f"Error processing file: {str(e)}")
        raise 
"#;
                // Create a PythonTool
                let python_tool = PythonTool {
                    name: "text-file-copy".to_string(),
                    homepage: None,
                    version: "1.0.0".to_string(),
                    author: "Shinkai Test".to_string(),
                    py_code: python_code.to_string(),
                    tools: vec![],
                    config: vec![],
                    description: "Reads a text file and returns its content with ' copy' appended".to_string(),
                    keywords: vec!["file".to_string(), "text".to_string(), "utility".to_string()],
                    input_args: Parameters::new(),
                    output_arg: ToolOutputArg { json: "".to_string() },
                    activated: true,
                    embedding: None,
                    result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
                    sql_tables: None,
                    sql_queries: None,
                    file_inbox: None,
                    oauth: None,
                    assets: None,
                    runner: RunnerType::OnlyHost,
                    operating_system: vec![OperatingSystem::Linux, OperatingSystem::MacOS, OperatingSystem::Windows],
                    tool_set: None,
                };

                // Create a ShinkaiTool from the PythonTool
                let shinkai_tool = ShinkaiTool::Python(python_tool, true);

                // Create a ShinkaiToolWithAssets
                let shinkai_tool_with_assets = ShinkaiToolWithAssets {
                    tool: shinkai_tool,
                    assets: None,
                };

                // Register the tool using V2ApiAddShinkaiTool
                let (res_sender, res_receiver) = async_channel::bounded(1);

                node1_commands_sender
                    .send(NodeCommand::V2ApiAddShinkaiTool {
                        bearer: api_key_bearer.clone(),
                        shinkai_tool: shinkai_tool_with_assets,
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                let tool_registration_result = res_receiver.recv().await.unwrap();
                match tool_registration_result {
                    Ok(response) => {
                        println!("Tool registered successfully: {:?}", response);

                        // Now create a job after the tool is registered
                        shinkai_log(
                            ShinkaiLogOption::Tests,
                            ShinkaiLogLevel::Debug,
                            &format!("Creating a Job for Agent {}", node1_agent.clone()),
                        );
                        let vector_fs_folder = ShinkaiPath::from_string("test_folder".to_string());

                        let job_scope = MinimalJobScope {
                            vector_fs_items: vec![],
                            vector_fs_folders: vec![vector_fs_folder],
                            vector_search_mode: VectorSearchMode::FillUpTo25k,
                        };

                        let job_id = api_create_job_with_scope(
                            node1_commands_sender.clone(),
                            clone_static_secret_key(&node1_profile_encryption_sk),
                            node1_encryption_pk,
                            clone_signature_secret_key(&node1_profile_identity_sk),
                            node1_identity_name,
                            node1_subidentity_name,
                            &node1_agent.clone(),
                            job_scope,
                        )
                        .await;

                        // Now execute the tool
                        let mut parameters = Map::new();
                        parameters.insert("file_path".to_string(), json!(test_file_path.to_str().unwrap()));

                        let tool_execution_result = api_execute_tool(
                            node1_commands_sender.clone(),
                            api_key_bearer.clone(),
                            "local:::text-file-copy:::text-file-copy".to_string(),
                            parameters,
                            "text-file-copy".to_string(),
                            "text-file-copy".to_string(),
                            node1_agent.to_string(),
                            Map::new(),
                            Map::new(),
                        )
                        .await;

                        // Handle the result
                        match tool_execution_result {
                            Ok(response) => {
                                // Process the successful response
                                println!("Tool executed successfully: {:?}", response);

                                // Verify the result
                                let content = response["content"].as_str().unwrap_or("");
                                let expected_content = format!("{} copy", test_content);
                                assert_eq!(content, expected_content, "Tool output should match expected content");

                                let original_file_path = response["original_file_path"].as_str().unwrap_or("");
                                assert_eq!(
                                    original_file_path,
                                    test_file_path.to_str().unwrap(),
                                    "Original file path should match"
                                );
                            }
                            Err(error) => {
                                // Handle the error
                                eprintln!("Tool execution failed: {:?}", error);
                                panic!("Tool execution failed: {:?}", error);
                            }
                        }
                    }
                    Err(error) => {
                        eprintln!("Tool registration failed: {:?}", error);
                        panic!("Tool registration failed: {:?}", error);
                    }
                }

                // Clean up the test file
                fs::remove_file(test_file_path).unwrap();

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
