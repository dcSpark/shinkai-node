use crate::it::utils::db_handlers::setup;
use serde_json::json;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::llm_provider::execution::chains::generic_chain::generic_inference_chain::GenericInferenceChain;
use shinkai_node::llm_provider::execution::chains::inference_chain_trait::MockInferenceChainContext;
use shinkai_node::llm_provider::providers::shared::openai::FunctionCall;
use shinkai_node::tools::js_toolkit::JSToolkit;
use shinkai_node::tools::shinkai_tool::ShinkaiTool;
use shinkai_node::tools::tool_router::ToolRouter;
use shinkai_tools_runner::built_in_tools;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use std::sync::Arc;

fn default_test_profile() -> ShinkaiName {
    ShinkaiName::new("@@alice.shinkai/profileName".to_string()).unwrap()
}

#[tokio::test]
async fn test_toolkit_installation_from_built_in_tools() {
    init_default_tracing();
    setup();

    // Initialize the database and profile
    let db_path = format!("db_tests/{}", "toolkit");
    let shinkai_db = Arc::new(ShinkaiDB::new(&db_path).unwrap());
    let profile = default_test_profile();
    let generator = RemoteEmbeddingGenerator::new_default();

    // Check and install built-in toolkits if not already installed
    let toolkit_list = shinkai_db.list_toolkits_for_user(&profile).unwrap();
    if toolkit_list.is_empty() {
        let tools = built_in_tools::get_tools();
        for (name, definition) in tools {
            let toolkit = JSToolkit::new(&name, definition);
            shinkai_db.add_jstoolkit(toolkit, profile.clone()).unwrap();
        }
    }

    // Verify that 4 toolkits were installed
    let toolkit_list = shinkai_db.list_toolkits_for_user(&profile).unwrap();
    for toolkit in &toolkit_list {
        println!("Toolkit name: {}", toolkit.name);
        for tool in &toolkit.tools {
            println!("  Tool name: {}", tool.name);
        }
    }
    eprintln!("toolkit_list.len(): {}", toolkit_list.len());
    assert_eq!(toolkit_list.len(), 4);

    // Activate all installed toolkits
    for toolkit in &toolkit_list {
        for tool in &toolkit.tools {
            let shinkai_tool = ShinkaiTool::JS(tool.clone());
            shinkai_db
                .activate_jstool(&shinkai_tool.tool_router_key(), &profile)
                .unwrap();
        }
    }

    // Initialize ToolRouter
    let mut tool_router = ToolRouter::new();
    tool_router
        .start(
            Box::new(generator.clone()),
            Arc::downgrade(&shinkai_db),
            profile.clone(),
        )
        .await
        .unwrap();

    // Perform a tool search for "weather" and check that one tool is returned
    let query = generator
        .generate_embedding_default("I want to know the weather in Austin")
        .await
        .unwrap();
    let results = tool_router.vector_search(&profile, query, 15).unwrap();
    // for toolkit in &toolkit_list {
    //     println!("Toolkit name: {}, description: {}", toolkit.name, toolkit.author);
    // }
    assert_eq!(results[0].name(), "Shinkai: Weather By City");
}

#[tokio::test]
async fn test_call_function_weather_by_city() {
    init_default_tracing();
    setup();

    // Initialize the database and profile
    let db_path = format!("db_tests/{}", "toolkit");
    let shinkai_db = Arc::new(ShinkaiDB::new(&db_path).unwrap());
    let profile = default_test_profile();
    let generator = RemoteEmbeddingGenerator::new_default();

    // Add built-in toolkits
    let tools = built_in_tools::get_tools();
    for (name, definition) in tools {
        let toolkit = JSToolkit::new(&name, definition);
        shinkai_db.add_jstoolkit(toolkit, profile.clone()).unwrap();
    }

    // Initialize ToolRouter
    let mut tool_router = ToolRouter::new();
    tool_router
        .start(
            Box::new(generator.clone()),
            Arc::downgrade(&shinkai_db),
            profile.clone(),
        )
        .await
        .unwrap();

    // Create a mock context
    let context = MockInferenceChainContext::default();

    // Define the function call
    let function_call = FunctionCall {
        name: "shinkai__web3_eth_balance".to_string(),
        arguments: json!({"address": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e"}),
    };

    // Find the tool with the name from the function_call
    let toolkit_list = shinkai_db.list_toolkits_for_user(&profile).unwrap();
    let mut shinkai_tool = None;
    for toolkit in &toolkit_list {
        for tool in &toolkit.tools {
            if tool.name == function_call.name {
                shinkai_tool = Some(ShinkaiTool::JS(tool.clone()));
                break;
            }
        }
        if shinkai_tool.is_some() {
            break;
        }
    }

    // Ensure the tool was found
    let shinkai_tool = shinkai_tool.expect("Tool not found");

    // Call the function using ToolRouter
    let result = tool_router
        .call_function(function_call, &context, &shinkai_tool.clone(), &profile)
        .await;

    // Check the result
    match result {
        Ok(response) => {
            println!("Function response: {}", response.response);
            assert!(response.response.contains("balance"));
            assert!(response.response.contains("ETH"));
        }
        Err(e) => panic!("Function call failed with error: {:?}", e),
    }
}
