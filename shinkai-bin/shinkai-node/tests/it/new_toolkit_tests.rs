use crate::it::utils::db_handlers::setup;
use serde_json::json;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::llm_provider::execution::chains::inference_chain_trait::MockInferenceChainContext;
use shinkai_node::llm_provider::providers::shared::openai::FunctionCall;
use shinkai_node::tools::js_toolkit::JSToolkit;
use shinkai_node::tools::shinkai_tool::ShinkaiTool;
use shinkai_node::tools::tool_router::ToolRouter;
use shinkai_tools_runner::built_in_tools;
use shinkai_tools_runner::tools::tool_definition::ToolDefinition;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use std::fs;
use std::path::Path;
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
            let toolkit = JSToolkit::new(&name, vec![definition]);
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
    assert!(
        toolkit_list.len() >= 4,
        "Expected at least 4 toolkits, but found {}",
        toolkit_list.len()
    );

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
    assert_eq!(results[0].name(), "shinkai__weather_by_city");
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
        let toolkit = JSToolkit::new(&name, vec![definition]);
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
        .call_function(function_call, shinkai_db, &context, &shinkai_tool.clone(), &profile)
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

#[tokio::test]
async fn test_get_default_tools() {
    init_default_tracing();
    setup();

    // Initialize the database and profile
    let db_path = format!("db_tests/{}", "default_tools");
    let shinkai_db = Arc::new(ShinkaiDB::new(&db_path).unwrap());
    let profile = default_test_profile();
    let generator = RemoteEmbeddingGenerator::new_default();

    // Install built-in toolkits
    let tools = built_in_tools::get_tools();
    for (name, definition) in tools {
        let toolkit = JSToolkit::new(&name, vec![definition]);
        shinkai_db.add_jstoolkit(toolkit, profile.clone()).unwrap();
    }

    // Verify that toolkits were installed
    let toolkit_list = shinkai_db.list_toolkits_for_user(&profile).unwrap();
    assert!(!toolkit_list.is_empty(), "No toolkits were installed");

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

    // Get default tools
    let default_tools = tool_router.get_default_tools(&profile).unwrap();

    // Check if the math expression evaluator is in the default tools
    let math_tool = default_tools
        .iter()
        .find(|tool| tool.name() == "shinkai__math_expression_evaluator");
    assert!(
        math_tool.is_some(),
        "Math expression evaluator tool not found in default tools"
    );

    // Optionally, print out all default tools
    for tool in &default_tools {
        println!("Default tool: {}", tool.name());
    }

    // Assert that we have at least one default tool (the math expression evaluator)
    assert!(!default_tools.is_empty(), "No default tools were returned");
}

#[tokio::test]
async fn test_create_update_and_read_toolkit() {
    init_default_tracing();
    setup();

    // Initialize the database and profile
    let db_path = format!("db_tests/{}", "toolkit_update");
    let shinkai_db = Arc::new(ShinkaiDB::new(&db_path).unwrap());
    let profile = default_test_profile();

    // Get built-in tools
    let tools = built_in_tools::get_tools();

    // Create a new toolkit with all built-in tools
    let toolkit = JSToolkit::new("TestToolkit", tools.into_iter().map(|(_, tool_def)| tool_def).collect());

    // Add the toolkit to the database
    shinkai_db.add_jstoolkit(toolkit, profile.clone()).unwrap();

    // Read the toolkit from the database
    let read_toolkit = shinkai_db.get_toolkit("TestToolkit", &profile).unwrap();
    let initial_tool_count = read_toolkit.tools.len();
    assert!(initial_tool_count > 0, "Toolkit should contain tools");

    // Update the first tool
    let mut updated_tool = read_toolkit.tools[0].clone();
    updated_tool.description = "Updated description".to_string();
    updated_tool.js_code = "function updatedTool() { return 'Updated function'; }".to_string();

    let shinkai_tool = ShinkaiTool::JS(updated_tool.clone());
    shinkai_db.add_shinkai_tool(shinkai_tool, profile.clone()).unwrap();

    // Read the toolkit again
    let updated_toolkit = shinkai_db.get_toolkit("TestToolkit", &profile).unwrap();
    assert_eq!(updated_toolkit.tools.len(), initial_tool_count);

    // Check that the first tool has been updated
    let first_tool = &updated_toolkit.tools[0];
    assert_eq!(first_tool.name, updated_tool.name);
    assert_eq!(first_tool.description, "Updated description");
    assert_eq!(
        first_tool.js_code,
        "function updatedTool() { return 'Updated function'; }"
    );

    // Check that other tools remain unchanged
    for (i, tool) in updated_toolkit.tools.iter().enumerate().skip(1) {
        assert_eq!(tool, &read_toolkit.tools[i], "Tool at index {} should be unchanged", i);
    }

    // Remove the toolkit
    shinkai_db.remove_jstoolkit("TestToolkit", &profile).unwrap();

    // Verify that the toolkit no longer exists
    assert!(shinkai_db.get_toolkit("TestToolkit", &profile).is_err());

    // Verify that the tools no longer exist
    for tool in &updated_toolkit.tools {
        let tool_key = ShinkaiTool::gen_router_key(tool.name.clone(), "TestToolkit".to_string());
        assert!(shinkai_db.get_shinkai_tool(&tool_key, &profile).is_err());
    }

    // Verify that all_tools_for_user returns an empty vector
    let remaining_tools = shinkai_db.all_tools_for_user(&profile).unwrap();
    assert!(
        remaining_tools.is_empty(),
        "No tools should remain after removing the toolkit"
    );
}

#[tokio::test]
async fn test_create_toolkit_from_file() {
    init_default_tracing();
    setup();

    // Initialize the database and profile
    let db_path = format!("db_tests/{}", "toolkit_from_file");
    let shinkai_db = Arc::new(ShinkaiDB::new(&db_path).unwrap());
    let profile = default_test_profile();

    // Read the echo_definition.json file
    let file_path = Path::new("../../files/echo_definition.json");
    let definition_json = fs::read_to_string(file_path).expect("Failed to read echo_definition.json");

    // Parse the toolkit file into ToolDefinition
    let tool_definition: ToolDefinition =
        serde_json::from_str(&definition_json).expect("Failed to parse JSON into ToolDefinition");

    // Validate that the code field is not None
    assert!(
        tool_definition.code.is_some(),
        "Tool definition is missing the code field"
    );

    // Create JSToolkit
    let toolkit = JSToolkit::new(&tool_definition.name, vec![tool_definition.clone()]);

    // Add the toolkit to the database
    shinkai_db.add_jstoolkit(toolkit, profile.clone()).unwrap();

    // Read the toolkit from the database
    let read_toolkit = shinkai_db.get_toolkit(&tool_definition.name, &profile).unwrap();

    // Verify that the toolkit was created correctly
    assert_eq!(read_toolkit.name, tool_definition.name);
    assert_eq!(read_toolkit.tools.len(), 1);

    let read_tool = &read_toolkit.tools[0];
    assert_eq!(read_tool.name, "shinkai__echo");
    assert_eq!(read_tool.description, tool_definition.description);
    assert_eq!(read_tool.js_code, tool_definition.code.unwrap());

    // Verify the result field
    assert_eq!(read_tool.result.result_type, tool_definition.result["type"].as_str().unwrap_or("object"));
    assert_eq!(read_tool.result.properties, tool_definition.result["properties"]);
    assert_eq!(
        read_tool.result.required,
        tool_definition.result["required"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<String>>())
            .unwrap_or_default()
    );

    // Verify that the tool can be activated
    let shinkai_tool = ShinkaiTool::JS(read_tool.clone());
    shinkai_db
        .activate_jstool(&shinkai_tool.tool_router_key(), &profile)
        .unwrap();

    // Verify that the tool is in the list of active tools
    let active_tools = shinkai_db.all_tools_for_user(&profile).unwrap();
    assert!(
        active_tools.iter().any(|t| t.name() == read_tool.name),
        "The new tool should be in the list of active tools"
    );

    // Clean up: remove the toolkit
    shinkai_db.remove_jstoolkit(&tool_definition.name, &profile).unwrap();
}

#[tokio::test]
async fn test_workflow_search() {
    init_default_tracing();
    setup();

    // Initialize the database and profile
    let db_path = format!("db_tests/{}", "workflow_search");
    let shinkai_db = Arc::new(ShinkaiDB::new(&db_path).unwrap());
    let profile = default_test_profile();
    let generator = RemoteEmbeddingGenerator::new_default();

    // Add built-in toolkits
    let tools = built_in_tools::get_tools();
    for (name, definition) in tools {
        let toolkit = JSToolkit::new(&name, vec![definition]);
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

    // Perform a workflow search
    let query = generator
        .generate_embedding_default("summarize this")
        .await
        .unwrap();
    let results = tool_router.workflow_search(profile, Box::new(generator), shinkai_db, query, "summarize this", 5).await.unwrap();

    // Assert the results
    assert!(!results.is_empty(), "Expected to find workflows, but found none");
    assert!(
        results.iter().any(|tool| tool.name().contains("ExtensiveSummary")),
        "Expected to find a workflow with 'example' in the name"
    );
    // // Optionally, print out the found workflows
    // for workflow in &results {
    //     println!("Found workflow: {}", workflow.name());
    // }
}
