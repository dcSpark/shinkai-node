use crate::it::utils::db_handlers::setup;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::tools::js_toolkit::JSToolkit;
use shinkai_node::tools::router::ToolRouter;
use shinkai_node::tools::shinkai_tool::ShinkaiTool;
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
        for (name, js_code) in tools {
            let toolkit = JSToolkit::new_semi_dummy_with_defaults(&name, &js_code);
            shinkai_db.add_jstoolkit(toolkit, profile.clone()).unwrap();
        }
    }

    // // Verify that 4 toolkits were installed
    // let toolkit_list = shinkai_db.list_toolkits_for_user(&profile).unwrap();
    // for toolkit in &toolkit_list {
    //     println!("Toolkit name: {}", toolkit.name);
    //     for tool in &toolkit.tools {
    //         println!("  Tool name: {}", tool.name);
    //     }
    // }
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
    let tool_router = ToolRouter::new(
        Box::new(generator.clone()),
        Arc::downgrade(&shinkai_db),
        profile.clone(),
    )
    .await
    .unwrap();

    // Perform a tool search for "weather" and check that one tool is returned
    let query = generator.generate_embedding_default("I want to know the weather in Austin").await.unwrap();
    let results = tool_router.vector_search(&profile, query, 15).unwrap();
    for toolkit in &toolkit_list {
        println!("Toolkit name: {}, description: {}", toolkit.name, toolkit.author);
    }
    assert_eq!(results[0].name(), "shinkai-tool-weather-by-city");
}
