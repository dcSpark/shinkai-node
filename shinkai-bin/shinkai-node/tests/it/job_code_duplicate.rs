use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, Ollama, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_tools_primitives::tools::deno_tools::DenoTool;
use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolWithAssets};
use shinkai_tools_primitives::tools::tool_config::ToolConfig;
use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
use shinkai_tools_primitives::tools::{
    parameters::Parameters,
    tool_playground::{ToolPlayground, ToolPlaygroundMetadata},
    tool_types::{OperatingSystem, RunnerType, ToolResult},
};

use utils::test_boilerplate::run_test_one_node_network;

use super::utils;
use super::utils::node_test_api::{
    api_initial_registration_with_no_code_for_device, api_llm_provider_registration, wait_for_default_tools,
};
use mockito::Server;

#[test]
fn tool_duplicate_tests() {
    std::env::set_var("WELCOME_MESSAGE", "false");

    let mut server = Server::new();

    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_llm_provider = env.node1_llm_provider.clone();
            let node1_encryption_pk = env.node1_encryption_pk.clone();
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
            let node1_api_key = env.node1_api_key.clone();
            let node1_abort_handler = env.node1_abort_handler;

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

                // Wait for default tools to be ready
                let tools_ready = wait_for_default_tools(
                    node1_commands_sender.clone(),
                    node1_api_key.clone(),
                    20, // Wait up to 30 seconds
                )
                .await
                .expect("Failed to check for default tools");
                assert!(tools_ready, "Default tools should be ready within 30 seconds");
            }

            {
                // Register an Agent
                eprintln!("\n\nRegister an Agent in Node1 and verify it");
                let agent_name = ShinkaiName::new(
                    format!(
                        "{}/{}/agent/{}",
                        node1_identity_name.clone(),
                        node1_profile_name.clone(),
                        node1_llm_provider.clone()
                    )
                    .to_string(),
                )
                .unwrap();

                // Note: this is mocked for Ollamas API
                // The code is non-valid, it's just a mock
                let _m = server
                    .mock("POST", "/api/chat")
                    .with_status(200)
                    .with_header("content-type", "application/json")
                    .with_body(
                        r#"{
                            "model": "mixtral:8x7b-instruct-v0.1-q4_1",
                            "created_at": "2023-12-19T11:36:44.687874415Z",
                            "message": {
                                "role": "assistant",
                                "content": "```typescript\nimport { getHomePath } from './shinkai-local-support.ts';\n\ntype CONFIG = {};\ntype INPUTS = {};\ntype OUTPUT = {};\n\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\n  const homeDir = await getHomePath();\n  console.log(`The Shinkai Node is running in the directory: ${homeDir}`);\n  return {};\n}```"
                            },
                            "done": true,
                            "total_duration": 29617027653,
                            "load_duration": 7157879293,
                            "prompt_eval_count": 203,
                            "prompt_eval_duration": 19022360000,
                            "eval_count": 25,
                            "eval_duration": 3435284000
                        }"#,
                    )
                    .create();

                let ollama = Ollama {
                    model_type: "mixtral:8x7b-instruct-v0.1-q4_1".to_string(),
                };

                let agent = SerializedLLMProvider {
                    id: node1_llm_provider.clone().to_string(),
                    full_identity_name: agent_name,
                    external_url: Some(server.url()),
                    api_key: Some("".to_string()),
                    model: LLMProviderInterface::Ollama(ollama),
                };
                api_llm_provider_registration(
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

            let tool_key_name: String = "local:::__node1_test_sep_shinkai:::demo_tool".to_string();
            println!("Creating tool");
            {
                // Create a tool offering
                let deno = DenoTool {
                    name: "demo_tool".to_string(),
                    homepage: Some("http://127.0.0.1/index.html".to_string()),
                    author: "@@node1_test.sep-shinkai".to_string(),
                    version: "1.0.0".to_string(),
                    mcp_enabled: Some(false),
                    js_code: "console.log('Hello, Deno 1!');".to_string(),
                    tools: vec![ToolRouterKey::from_string(
                        "local:::__official_shinkai:::shinkai_llm_prompt_processor",
                    )
                    .unwrap()],
                    config: vec![ToolConfig::from_value(&serde_json::json!({
                        "key_name": "a",
                        "key_value": "b",
                        "description": "c",
                        "required": true,
                        "type": null
                    }))
                    .unwrap()],
                    oauth: None,
                    description: "A Deno tool for testing 1".to_string(),
                    keywords: vec!["deno".to_string(), "test".to_string()],
                    input_args: Parameters::with_single_property(
                        "prompt".to_string().as_str(),
                        "string".to_string().as_str(),
                        "The prompt to process".to_string().as_str(),
                        true,
                    ),
                    activated: true,
                    embedding: None,
                    result: ToolResult::new(
                        "object".to_string(),
                        serde_json::json!({
                            "result": { "type": "string", "description": "The result" },
                            "count": { "type": "number", "description": "Count value" }
                        }),
                        vec!["result".to_string()],
                    ),
                    output_arg: ToolOutputArg {
                        json: r#"{
                            "result": { "type": "string", "description": "The result" },
                            "count": { "type": "number", "description": "Count value" }
                        }"#
                        .to_string(),
                    },
                    sql_tables: Some(vec![]),
                    sql_queries: Some(vec![]),
                    file_inbox: None,
                    assets: None,
                    runner: RunnerType::OnlyHost,
                    operating_system: vec![OperatingSystem::Windows],
                    tool_set: None,
                };
                eprintln!("\nCreate a tool");
                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiAddShinkaiTool {
                        bearer: node1_api_key.clone(),
                        shinkai_tool: ShinkaiToolWithAssets {
                            tool: ShinkaiTool::Deno(deno, true),
                            assets: None,
                        },
                        res: res_sender,
                    })
                    .await
                    .unwrap();
                let result = res_receiver.recv().await.unwrap();
                assert!(result.is_ok(), "Tool addition failed");
                eprintln!("Tool key name: {:?}", result);
            }

            let tool_key_name_duplicate = {
                // Fork the tool implementation
                eprintln!("\nDuplicate the tool implementation");
                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiDuplicateTool {
                        bearer: node1_api_key.clone(),
                        tool_key_path: tool_key_name.clone(),
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                let result = res_receiver.recv().await.unwrap();
                eprintln!("Duplicate result: {:?}", result);
                assert!(result.is_ok(), "Tool fork failed");
                let fork_result = result.unwrap();
                eprintln!("Fork result: {:?}", fork_result);
                // Fork result: Object {"job_id": String("jobid_5ae1f3e2-874a-47a1-b98a-23cd255f0456"), "tool_router_key": String("local:::__node1_test_sep_shinkai:::demo_tool_20250331_163342"), "version": String("1.0.0")}
                fork_result
                    .get("tool_router_key")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            };

            {
                eprintln!("Checking the tool is duplicated");
                let (res_sender, res_receiver) = async_channel::bounded(1);
                // Check the tool is duplicated
                node1_commands_sender
                    .send(NodeCommand::V2ApiGetShinkaiTool {
                        bearer: node1_api_key.clone(),
                        res: res_sender,
                        serialize_config: false,
                        payload: tool_key_name_duplicate.clone(),
                    })
                    .await
                    .unwrap();

                let result = res_receiver.recv().await.unwrap();
                eprintln!("Get tool offering result: {:?}", result);
                assert!(result.is_ok(), "Tool fork failed");
                let fork_result = result.unwrap();
                eprintln!("Duplicate result: {:?}", fork_result);
            }

            {
                eprintln!("Checking the tool playground is duplicated");
                let (res_sender, res_receiver) = async_channel::bounded(1);
                // Check the tool is duplicated
                node1_commands_sender
                    .send(NodeCommand::V2ApiGetPlaygroundTool {
                        bearer: node1_api_key.clone(),
                        res: res_sender,
                        tool_key: tool_key_name_duplicate.clone(),
                    })
                    .await
                    .unwrap();

                let result = res_receiver.recv().await.unwrap();
                eprintln!("Get tool offering result: {:?}", result);
                assert!(result.is_ok(), "Tool fork failed");
                let fork_result = result.unwrap();
                // Get tool offering result: Ok(Object {"assets": Null, "code": String("console.log('Hello, Deno 1!');"), "job_id": String("jobid_9b3a96e9-7cf3-43b6-9306-f5acb56ca27c"), "job_id_history": Array [String("")], "language": String("typescript"), "metadata": Object {"author": String("@@node1_test.sep-shinkai"), "configurations": Object {"properties": Object {"a": Object {"description": String("c"), "type": String("string")}}, "required": Array [String("a")], "type": String("object")}, "description": String("A Deno tool for testing 1"), "homepage": String("http://127.0.0.1/index.html"), "keywords": Array [String("deno"), String("test")], "name": String("demo_tool_20250331_164114"), "oauth": Null, "operating_system": Array [String("windows")], "parameters": Object {"properties": Object {"prompt": Object {"description": String("The prompt to process"), "type": String("string")}}, "required": Array [String("prompt")], "type": String("object")}, "result": Object {"properties": Null, "required": Array [], "type": String("object")}, "runner": String("only_host"), "sqlQueries": Array [], "sqlTables": Array [], "tool_set": Null, "tools": Array [String("local:::__official_shinkai:::shinkai_llm_prompt_processor")], "version": String("1.0.0")}, "tool_router_key": String("local:::__node1_test_sep_shinkai:::demo_tool_20250331_164114")})

                let language = fork_result.get("language").unwrap().as_str().unwrap();
                eprintln!("Language: {:?}", language);
                let code = fork_result.get("code").unwrap().as_str().unwrap();
                eprintln!("Code: {:?}", code);
                let metadata = fork_result.get("metadata").unwrap().as_object().unwrap();
                eprintln!("Metadata: {:?}", metadata);
                // Metadata: {
                //     "author": String("@@node1_test.sep-shinkai"),
                //     "configurations": Object {
                //         "properties": Object {
                //             "a": Object {"description": String("c"), "type": String("string")}},
                //         "required": Array [String("a")],
                //         "type": String("object")},
                //     "description": String("A Deno tool for testing 1"),
                //     "homepage": String("http://127.0.0.1/index.html"),
                //     "keywords": Array [String("deno"), String("test")],
                //     "name": String("demo_tool_20250331_164957"),
                //     "oauth": Null,
                //     "operating_system": Array [String("windows")],
                //     "parameters": Object {
                //         "properties": Object {
                //             "prompt": Object {"description": String("The prompt to process"), "type": String("string")}},
                //         "required": Array [String("prompt")],
                //         "type": String("object")},
                //     "result": Object {"properties": Null, "required": Array [], "type": String("object")},
                //     "runner": String("only_host"),
                //     "sqlQueries": Array [], "sqlTables": Array [], "tool_set": Null, "tools": Array [String("local:::__official_shinkai:::shinkai_llm_prompt_processor")], "version": String("1.0.0")}
                let author = metadata.get("author").unwrap().as_str().unwrap();
                eprintln!("Author: {:?}", author);
                assert_eq!(author, "@@node1_test.sep-shinkai");

                let configurations = metadata.get("configurations").unwrap().as_object().unwrap();
                eprintln!("Configurations: {:?}", configurations);
                assert!(configurations.get("properties").unwrap().is_object());

                let configurations_properties: &serde_json::Map<String, serde_json::Value> =
                    configurations.get("properties").unwrap().as_object().unwrap();
                eprintln!("Properties: {:?}", configurations_properties);

                let configurations_properties_a = configurations_properties.get("a").unwrap().as_object().unwrap();
                eprintln!("A: {:?}", configurations_properties_a);

                let configurations_properties_a_type =
                    configurations_properties_a.get("type").unwrap().as_str().unwrap();
                eprintln!("Type: {:?}", configurations_properties_a_type);
                assert!(configurations_properties_a_type.to_string().contains("string"));

                let configurations_properties_a_description = configurations_properties_a
                    .get("description")
                    .unwrap()
                    .as_str()
                    .unwrap();
                eprintln!("Description: {:?}", configurations_properties_a_description);
                assert!(configurations_properties_a_description.to_string().contains("c"));
                assert_eq!(configurations_properties_a_description, "c");

                let configurations_properties_a_type =
                    configurations_properties_a.get("type").unwrap().as_str().unwrap();
                assert_eq!(configurations_properties_a_type, "string");
                let configurations_properties_a_type =
                    configurations_properties_a.get("type").unwrap().as_str().unwrap();
                assert_eq!(configurations_properties_a_type, "string");

                let configurations_required = configurations
                    .get("required")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<&str>>();
                eprintln!("Required: {:?}", configurations_required);

                let description = metadata.get("description").unwrap().as_str().unwrap();
                eprintln!("Description: {:?}", description);
                assert!(description.to_string().contains("A Deno tool"));

                let home_page = metadata.get("homepage").unwrap().as_str().unwrap();
                eprintln!("Home page: {:?}", home_page);
                assert!(home_page.to_string().contains("http://127.0.0.1/index.html"));

                let keywords = metadata
                    .get("keywords")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<&str>>();
                eprintln!("Keywords: {:?}", keywords);
                assert!(keywords.contains(&"deno"));
                assert!(keywords.contains(&"test"));

                let name = metadata.get("name").unwrap().as_str().unwrap();
                eprintln!("Name: {:?}", name);
                assert!(name.to_string().contains("demo_tool"));

                let parameters = metadata.get("parameters").unwrap().as_object().unwrap();
                eprintln!("Parameters: {:?}", parameters);

                let properties = parameters.get("properties").unwrap().as_object().unwrap();
                eprintln!("Properties: {:?}", properties);

                let prompt = properties.get("prompt").unwrap().as_object().unwrap();
                eprintln!("Prompt: {:?}", prompt);

                let prompt_description = prompt.get("description").unwrap().as_str().unwrap();
                eprintln!("Description: {:?}", prompt_description);
                assert!(prompt_description.to_string().contains("The prompt to process"));

                let prompt_type = prompt.get("type").unwrap().as_str().unwrap();
                eprintln!("Type: {:?}", prompt_type);
                assert!(prompt_type.to_string().contains("string"));
                assert_eq!(prompt_type, "string");

                let parameters_required = parameters
                    .get("required")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<&str>>();
                eprintln!("Required: {:?}", parameters_required);
                assert!(parameters_required.contains(&"prompt"));

                let result = metadata.get("result").unwrap().as_object().unwrap();
                eprintln!("Result: {:?}", result);

                let properties = result.get("properties").unwrap().as_object().unwrap();
                eprintln!("Properties: {:?}", properties);

                let properties_result_type = properties.get("result").unwrap().get("type").unwrap().as_str().unwrap();
                eprintln!("Type: {:?}", properties_result_type);
                assert!(properties_result_type.to_string().contains("string"));

                let runner = metadata.get("runner").unwrap().as_str().unwrap();
                eprintln!("Runner: {:?}", runner);
                assert!(runner.to_string().contains("only_host"));

                let sql_queries = metadata.get("sqlQueries").unwrap().as_array().unwrap();
                eprintln!("SQL Queries: {:?}", sql_queries);

                let sql_tables = metadata.get("sqlTables").unwrap().as_array().unwrap();
                eprintln!("SQL Tables: {:?}", sql_tables);

                let tool_set = metadata.get("tool_set");
                eprintln!("Tool set: {:?}", tool_set);
                assert!(tool_set.is_none() || tool_set.unwrap().is_null());

                let tools = metadata.get("tools").unwrap().as_array().unwrap();
                eprintln!("Tools: {:?}", tools);

                let version = metadata.get("version").unwrap().as_str().unwrap();
                eprintln!("Version: {:?}", version);

                let oauth = metadata.get("oauth");
                eprintln!("OAuth: {:?}", oauth);
                assert!(oauth.is_none() || oauth.unwrap().is_null());

                let operating_system = metadata
                    .get("operating_system")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<&str>>();
                eprintln!("Operating system: {:?}", operating_system);
                assert!(operating_system.contains(&"windows"));
            }

            node1_abort_handler.abort();
        })
    });
}
