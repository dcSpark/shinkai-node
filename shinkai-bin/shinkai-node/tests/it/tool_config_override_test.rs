use async_channel::{bounded, Receiver, Sender};
use rand::Rng;
use serde_json::{json, Map, Value};
use shinkai_http_api::node_api_router;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::agent::Agent;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, OpenAI, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, hash_signature_public_key, unsafe_deterministic_signature_keypair
};
use shinkai_node::network::Node;
use shinkai_node::tools::tool_implementation::native_tools::sql_processor::get_database_path_from_db_name_config;
use shinkai_tools_primitives::tools::deno_tools::DenoTool;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolWithAssets};
use shinkai_tools_primitives::tools::tool_config::{BasicConfig, ToolConfig};
use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
use shinkai_tools_primitives::tools::tool_playground::{SqlQuery, SqlTable};
use shinkai_tools_primitives::tools::tool_types::{OperatingSystem, RunnerType, ToolResult};
use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use tokio::runtime::Runtime;

use crate::it::utils::db_handlers::setup;
use crate::it::utils::node_test_api::{
    api_execute_tool, api_registration_device_node_profile_main, wait_for_default_tools,
};
use crate::it::utils::test_boilerplate::{default_embedding_model, supported_embedding_models};

use mockito::Server;

#[test]
fn test_tool_execution_with_config_override() {
    setup();

    std::env::set_var("WELCOME_MESSAGE", "false");
    let api_key_bearer = std::env::var("API_V2_KEY").unwrap_or_else(|_| "my_api_v2_key".to_string());
    std::env::set_var("API_V2_KEY", api_key_bearer.clone());
    std::env::set_var("NODE_API_PORT", "9550");
    std::env::set_var("SKIP_IMPORT_FROM_DIRECTORY", "true");
    std::env::set_var("IS_TESTING", "1");
    let node1_db_path = format!("db_tests/{}", hash_signature_public_key(&unsafe_deterministic_signature_keypair(0).1));
    println!("node1_db_path: {:?}", node1_db_path);
    std::env::set_var("NODE_STORAGE_PATH", node1_db_path.clone());

    let rt = Runtime::new().unwrap();
    let server = Server::new();
    fn port_is_available(port: u16) -> bool {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    let e = rt.block_on(async {
        let node1_identity_name  = "@@node1_test.sep-shinkai";
        let node1_subidentity_name = "main";
        let node1_device_name = "node1_device";
        let node1_agent_id = "node1_gpt_agent";
        let node1_llm_provider_id = "node1_llm_provider";

        let (node1_identity_sk, _node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let (node1_profile_identity_sk, _node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, _node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node1_device_identity_sk, _node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, _node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        // Create the LLM provider
        let agent_name = ShinkaiName::new(
            format!(
                "{}/{}/agent/{}",
                node1_identity_name, node1_subidentity_name, node1_agent_id
            )
            .to_string(),
        )
        .unwrap();

        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo-1106".to_string(),
        };

        let llm_provider = SerializedLLMProvider {
            id: node1_llm_provider_id.to_string(),
            full_identity_name: agent_name.clone(),
            name: Some("Test Agent".to_string()),
            description: Some("Test Agent Description".to_string()),
            external_url: Some(server.url()),
            api_key: Some("mockapikey".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
        };
        assert!(port_is_available(8080), "Port 8080 is not available");
        // Create node
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk,
            None,
            None,
            0,
            node1_commands_receiver,
            node1_db_path.clone(),
            "".to_string(),
            None,
            true,
            vec![llm_provider],
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

        // Register device node profile main
        api_registration_device_node_profile_main(
            node1_commands_sender.clone(),
            "main",
            &node1_identity_name,
            node1_encryption_pk,
            node1_device_encryption_sk.clone(),
            node1_device_identity_sk.clone(),
            node1_profile_encryption_sk.clone(),
            node1_profile_identity_sk.clone(),
            &node1_device_name,
        )
        .await;

        // Create node1 and node2
        assert!(port_is_available(9550), "Port 9550 is not available");
        assert!(port_is_available(9560), "Port 9560 is not available");
        // Setup API Server task
        let api_listen_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9550);
        let api_https_listen_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9560);

        let node1_commands_sender_clone = node1_commands_sender.clone();
        let _api_server = tokio::spawn(async move {
            if let Err(e) = node_api_router::run_api(
                node1_commands_sender_clone,
                api_listen_address,
                api_https_listen_address,
                node1_identity_name.to_string(),
                None,
                None,
            )
            .await
            {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    &format!("API server failed to start: {}", e),
                );
                panic!("API server failed to start: {}", e);
            }
        });

        // // Wait for default tools to be ready
        wait_for_default_tools(node1_commands_sender.clone(), api_key_bearer.clone(), 20)
            .await
            .unwrap();

        eprintln!("\nCreate a tool");
        let (res_sender, res_receiver) = async_channel::bounded(1);
        let mut params = Parameters::new();
        params.add_property("data".to_string(), "string".to_string(), "The data to process for memory management, if not provided, the tool will return existing memories".to_string(), false, None);
        params.add_property("specific_prompt".to_string(), "string".to_string(), "Optional. The specific prompt for generating memories. Default: 'Synthesize important information to remember from this interaction'".to_string(), false, None);
        params.add_property("memory_key".to_string(), "string".to_string(), "Optional. The Hashmap 'key' for specific memory retrieval. For example a user name or email.".to_string(), false, None);
        params.add_property("general_prompt".to_string(), "string".to_string(), "Optional. The general prompt for generating memories. Default: 'Synthesize important information to remember from this interaction'".to_string(), false, None);
        
        node1_commands_sender
            .send(NodeCommand::V2ApiAddShinkaiTool {
                bearer: api_key_bearer.clone(),
                shinkai_tool: ShinkaiToolWithAssets {
                    tool: ShinkaiTool::Deno(DenoTool {
                        activated: true,
                        embedding: None,
                        tool_router_key: Some(ToolRouterKey::from_string(   "local:::__official_shinkai:::memory_management").unwrap()),
                        name: "Memory Management".to_string(),
                        homepage: None,
                        author: "@@official.shinkai".to_string(),
                        version: "1.0.0".to_string(),
                        mcp_enabled: None,
                        js_code: "import { shinkaiSqliteQueryExecutor as shinkaiSqliteQueryExecutor_ } from \"./shinkai-local-tools.ts\";\nimport { shinkaiLlmPromptProcessor } from \"./shinkai-local-tools.ts\";\n\nconst shinkaiSqliteQueryExecutor = (params: any) => {\n  console.log(\"shinkaiSqliteQueryExecutor\", params);\n  return shinkaiSqliteQueryExecutor_(params);\n};\n\ntype CONFIG = {\n  database_name?: string;\n};\ntype INPUTS = {\n  data?: string;\n  general_prompt?: string;\n  specific_prompt?: string;\n  key?: string;\n};\ntype OUTPUT = {\n  generalMemory: string;\n  specificMemory: string;\n};\n\nconst createTable = async (\n  database_name: string | undefined\n): Promise<void> => {\n  // Create table if not exists\n  const createTableQuery = `\n        CREATE TABLE IF NOT EXISTS memory_table (\n            id INTEGER PRIMARY KEY AUTOINCREMENT,\n            date DATETIME DEFAULT CURRENT_TIMESTAMP,\n            key TEXT,\n            memory TEXT\n        );\n    `;\n  await shinkaiSqliteQueryExecutor({\n    query: createTableQuery,\n    ...(database_name && { database_name }),\n  });\n};\n\nconst getGeneralMemory = async (\n  database_name: string | undefined\n): Promise<null | { id: number; key: string; memory: string }> => {\n  const fetchGeneralMemoryQuery = `\n      SELECT id, key, memory\n      FROM memory_table\n      where key is null\n    `;\n  const fetchGeneralMemory = await shinkaiSqliteQueryExecutor({\n    query: fetchGeneralMemoryQuery,\n    ...(database_name && { database_name }),\n  });\n\n  if (fetchGeneralMemory.result.length) {\n    return fetchGeneralMemory.result[0];\n  }\n  return null;\n};\n\nconst getSpecificMemory = async (\n  database_name: string | undefined,\n  key: string\n): Promise<null | { id: number; key: string; memory: string }> => {\n  const fetchSpecificMemoryQuery = `\n      SELECT id, key, memory\n      FROM memory_table\n      where key = ?\n    `;\n  const fetchSpecificMemory = await shinkaiSqliteQueryExecutor({\n    query: fetchSpecificMemoryQuery,\n    params: [key],\n    ...(database_name && { database_name }),\n  });\n\n  if (fetchSpecificMemory.result.length) {\n    return fetchSpecificMemory.result[0];\n  }\n  return null;\n};\n\nconst generatePrompt = async (\n  previousMemory: null | { id: number; key: string; memory: string },\n  general_prompt: string,\n  data: string\n): Promise<string> => {\n  let prompt = `\nThere are two actions you can perform, depending on the \"input\" tag contents type:\n1. It's new information to remember.\n2. It's an imperative instruction.\n\nDepending on the \"input\" tag you must decide what action to perform.\nIf it's imperative, then follow the action-receive-imperative-instruction tag.\n\n<action-receive-information>\n* You must update your own memories, so we can recall new and past interactions.\n* You have access to your own memories, and you can merge them with the new information.\n* We should merge new and past interactions, into a single memory.\n* We can restructure the memory to make it consistent and ordered.\n* Keep the most important information only.\n* Based on the rules tag, you must generate the output.\n</action-receive-information>\n\n<action-receive-imperative-instruction>\nIf you receive an imperative instruction as:\n* clear all memories\n* forget something specific\n* update a specific memory\nYou must apply them to your memories.\n</action-receive-imperative-instruction>\n\n<formatting>\nUse \"##\" to write and identify main topics\nUse \"#\" to identify titles of definitions\n\nOnly output the new memory, without comments, suggestions or how it was generated.\nEverything you output will replace the previous memory.\nSo if you remove information from the output, it will be forgotten.\n</formatting>\n\n<memory-example>\nThis is an example on how to structure the memory, not the fields you must use.\n\\`\\`\\`\n# Location\n## NY: Latitude: 40.7128, Longitude: -74.0060\n## CO: Latitude: -33.4569, Longitude: -70.6483\n- CO has borders with Perú and Bolivia\n\n# Known People\n## John: 30 years old\n## Jane: 25 years old\n## Peter: is from Europe.\n- John and Jane are friends \n\\`\\`\\`\n</memory-example>\n\n<sections>\nThese are some sections you must understand:\n  * rules tag: has the rules you must follow to generate the output.\\n`;\n  if (previousMemory) prompt += '  * previous_interactions tag: has entire previous interaction memory\\n';\n\n  prompt += `  * input tag: has the new data or imperative instructions.;\n</sections>\n\n<rules>\n  ${general_prompt}\n</rules>\n    `;\n  if (previousMemory)\n    prompt += `\n<previous_interactions>\n  ${previousMemory.memory}\n</previous_interactions>\n      `;\n\n      prompt += `\n<input>\n  ${data}\n</input>\n    `;\n  return prompt;\n};\n\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\n  const {\n    data,\n    general_prompt = \"Synthesize important information to remember from this interaction\",\n    specific_prompt = \"Synthesize important information to remember from this interaction\",\n    key,\n  } = inputs;\n\n  await createTable(config.database_name);\n  // If no data provided, just return existing memories\n  if (!data) {\n    const existingGeneralMemory = await getGeneralMemory(config.database_name);\n    const existingSpecificMemory = key\n      ? await getSpecificMemory(config.database_name, key)\n      : null;\n\n    return {\n      generalMemory: existingGeneralMemory?.memory || \"\",\n      specificMemory: existingSpecificMemory?.memory || \"\",\n    };\n  }\n\n  if (!key) {\n    // Update General Memory\n    const previousGeneralMemory = await getGeneralMemory(config.database_name);\n    const generalPrompt = await generatePrompt(\n      previousGeneralMemory,\n      general_prompt,\n      data\n    );\n    const generalResponse = await shinkaiLlmPromptProcessor({\n      format: \"text\",\n      prompt: generalPrompt,\n    });\n    const generalMemory = generalResponse.message;\n\n    if (previousGeneralMemory) {\n      const generalUpdateQuery = `\n              UPDATE memory_table SET memory = ?\n              WHERE id = ?\n          `;\n      await shinkaiSqliteQueryExecutor({\n        query: generalUpdateQuery,\n        params: [generalMemory, \"\" + previousGeneralMemory.id],\n        ...(config.database_name && { database_name: config.database_name }),\n      });\n    } else {\n      const generalInsertQuery = `\n            INSERT INTO memory_table (memory)\n            VALUES (?);\n        `;\n      await shinkaiSqliteQueryExecutor({\n        query: generalInsertQuery,\n        params: [generalMemory],\n        ...(config.database_name && { database_name: config.database_name }),\n      });\n    }\n    return { generalMemory, specificMemory: \"\" };\n  } else {\n    // Update specific memory\n    const previousSpecificMemory = await getSpecificMemory(\n      config.database_name,\n      key\n    );\n    const specificPrompt = await generatePrompt(\n      previousSpecificMemory,\n      specific_prompt,\n      data\n    );\n    const specificResponse = await shinkaiLlmPromptProcessor({\n      format: \"text\",\n      prompt: specificPrompt,\n    });\n    const specificMemory = specificResponse.message;\n\n    if (previousSpecificMemory) {\n      const specificUpdateQuery = `\n            UPDATE memory_table SET memory = ?\n            WHERE id = ?\n        `;\n      await shinkaiSqliteQueryExecutor({\n        query: specificUpdateQuery,\n        params: [specificMemory, \"\" + previousSpecificMemory.id],\n        ...(config.database_name && { database_name: config.database_name }),\n      });\n    } else {\n      const specificInsertQuery = `\n            INSERT INTO memory_table (key, memory)\n            VALUES (?, ?);\n        `;\n      await shinkaiSqliteQueryExecutor({\n        query: specificInsertQuery,\n        params: [key, specificMemory],\n        ...(config.database_name && { database_name: config.database_name }),\n      });\n    }\n    return { generalMemory: \"\", specificMemory };\n  }\n}\n".to_string(),
                        tools: vec![
                          ToolRouterKey::from_string("local:::__official_shinkai:::shinkai_sqlite_query_executor").unwrap(),
                          ToolRouterKey::from_string("local:::__official_shinkai:::shinkai_llm_prompt_processor").unwrap()
                        ],
                        config: vec![                      
                            ToolConfig::BasicConfig(BasicConfig {
                              key_name: "database_name".to_string(),
                              description: "By default, the database name is the app_id. You can specify a different name to share the same database in multiple contexts.".to_string(),
                              required: false,
                              type_name: None,
                              key_value: None
                            })
                        ],
                        description: "Handles memory storage and retrieval using a database. It has two types of memories: general and specific. Specific memories are stored in a Hashmap, and can be retrieved by a key.".to_string(),
                        keywords: vec![
                          "memory".to_string(),
                          "remember".to_string(),
                          "management".to_string(),
                          "recall".to_string(),
                          "smart".to_string(),
                          "agent".to_string()
                        ],
                        input_args: params,
                        output_arg: ToolOutputArg { json: "".to_string() },
                        result: ToolResult {
                          r#type: "object".to_string(),
                          properties: json!({
                            "generalMemory": {
                              "description": "The updated general memory",
                              "nullable": true,
                              "type": "string"
                            },
                            "specificMemory": {
                              "description": "The updated specific memory",
                              "nullable": true,
                              "type": "string"
                            }
                          }),
                          required: vec![],
                        },
                        sql_tables: Some(   vec![
                            SqlTable {
                            name: "memory_table".to_string(),
                            definition: "CREATE TABLE IF NOT EXISTS memory_table (id INTEGER PRIMARY KEY AUTOINCREMENT, date DATETIME DEFAULT CURRENT_TIMESTAMP, key TEXT, memory TEXT)".to_string()
                          }
                        ]),
                        sql_queries: Some(vec![
                            SqlQuery {
                            name: "Get general memory".to_string(),
                            query: "SELECT id, key, memory FROM memory_table WHERE key IS NULL".to_string()
                          },
                          SqlQuery{
                            name: "Get specific memory".to_string(),
                            query: "SELECT id, key, memory FROM memory_table WHERE key = ?".to_string()
                          },
                          SqlQuery{
                            name: "Update memory".to_string(),
                            query: "UPDATE memory_table SET memory = ? WHERE id = ?".to_string()
                          }
                        ]),
                        file_inbox: None,
                        oauth: None,
                        assets: None,
                        runner: RunnerType::Any,
                        operating_system: vec![OperatingSystem::Linux, OperatingSystem::MacOS, OperatingSystem::Windows],
                        tool_set: Some("".to_string())
                      }, true),
                    assets: None,
                },
                res: res_sender,
            })
            .await
            .unwrap();
        let result = res_receiver.recv().await.unwrap();
        assert!(result.is_ok(), "Tool addition failed");
        eprintln!("Tool key name: {:?}", result);

        let random_database_name = format!("potato_database_{}", rand::thread_rng().gen_range(0..1000000));
        // Create a new agent with tools_config_override
        let mut tools_config_override = HashMap::new();
        let mut config_values = HashMap::new();
        config_values.insert("database_name".to_string(), Value::String(random_database_name.clone()));
        tools_config_override.insert(
            "local:::__official_shinkai:::memory_management".to_string(),
            config_values,
        );

        // Create the agent payload
        let agent_data = json!({
            "name": "Test Agent",
            "agent_id": node1_agent_id,
            "full_identity_name": agent_name.clone(),
            "llm_provider_id": node1_llm_provider_id,
            "ui_description": "Test agent description",
            "knowledge": [],
            "storage_path": "/test/storage/path",
            "tools": ["local:::__official_shinkai:::memory_management:::1.0.0"],
            "debug_mode": true,
            "tools_config_override": tools_config_override
        });

        let agent: Agent = serde_json::from_value(agent_data).unwrap();

        // Add the agent
        let (res_sender, res_receiver) = async_channel::bounded(1);
        node1_commands_sender
            .send(NodeCommand::V2ApiAddAgent {
                bearer: api_key_bearer.clone(),
                agent,
                res: res_sender,
            })
            .await
            .unwrap();

        // Verify the agent was added
        match res_receiver.recv().await.unwrap() {
            Ok(_) => eprintln!("Agent added successfully"),
            Err(e) => panic!("Failed to add agent: {:?}", e),
        }

        // Create parameters for tool execution
        let mut parameters = Map::new();
        parameters.insert("memory_key".to_string(), Value::String("potato key value".to_string()));

        // Execute the tool with the agent
        let result = api_execute_tool(
            node1_commands_sender.clone(),
            api_key_bearer.clone(),
            "local:::__official_shinkai:::memory_management".to_string(),
            parameters,
            "shinkai__echo".to_string(),
            "test_app".to_string(),
            Some(node1_agent_id.to_string()),
            node1_llm_provider_id.to_string(),
            Map::new(),
            Map::new(),
        )
        .await;

        abort_handler.abort();

        assert!(result.is_ok(), "Tool execution should succeed");

        // Verify the shared SQLite database exists
        let db_file_path = get_database_path_from_db_name_config(random_database_name).unwrap();

        assert!(
            std::path::Path::new(&db_file_path).exists(),
            "Shared SQLite database should exist at {}",
            db_file_path.display()
        );

        let result = tokio::try_join!(node1_handler);
        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                // Check if the error is because one of the tasks was aborted
                if e.is_cancelled() {
                    eprintln!("One of the tasks was aborted, but this is expected.");
                    Ok(())
                } else {
                    // If the error is not due to an abort, then it's unexpected
                    Err(e)
                }
            }
        }
    });

    rt.shutdown_timeout(Duration::from_secs(10));
    if let Err(e) = e {
        assert!(false, "An unexpected error occurred: {:?}", e);
    }
}
