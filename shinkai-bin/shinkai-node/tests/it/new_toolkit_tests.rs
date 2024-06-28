use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::tools::js_toolkit::JSToolkit;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_tools_runner::built_in_tools;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;

use crate::it::utils::db_handlers::setup;

fn default_test_profile() -> ShinkaiName {
    ShinkaiName::new("@@alice.shinkai/profileName".to_string()).unwrap()
}

#[tokio::test]
async fn test_toolkit_installation_from_built_in_tools() {
    init_default_tracing();
    setup();

    // Initialize the database and profile
    let db_path = format!("db_tests/{}", "toolkit");
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    let profile = default_test_profile();
    let generator = RemoteEmbeddingGenerator::new_default();
    shinkai_db
        .init_profile_tool_structs(&profile, Box::new(generator.clone()))
        .await
        .unwrap();

    // Check and install built-in toolkits if not already installed
    let toolkit_map = shinkai_db.get_installed_toolkit_map(&profile).unwrap();
    if toolkit_map.get_all_toolkit_infos().is_empty() {
        let tools = built_in_tools::get_tools();
        for (name, js_code) in tools {
            let toolkit = JSToolkit::new_semi_dummy_with_defaults(&name, &js_code);
            shinkai_db.install_toolkit(&toolkit, &profile).unwrap();
        }
    }

    // Verify that 4 toolkits were installed
    let toolkit_map = shinkai_db.get_installed_toolkit_map(&profile).unwrap();
    eprintln!("{:?}", toolkit_map.get_all_toolkit_infos());
    assert_eq!(toolkit_map.get_all_toolkit_infos().len(), 4);

    // Activate all installed toolkits
    // let executor = JSToolkitExecutor::new_local().await.unwrap();
    for toolkit_info in toolkit_map.get_all_toolkit_infos() {
        shinkai_db
            .activate_toolkit(&toolkit_info.name, &profile, Box::new(generator.clone()))
            .await
            .unwrap();
    }

    // Verify that all toolkits are activated
    let toolkit_map = shinkai_db.get_installed_toolkit_map(&profile).unwrap();
    eprintln!("\n\ntoolkit_map: {:?}", toolkit_map.get_all_toolkit_infos());
    for toolkit_info in toolkit_map.get_all_toolkit_infos() {
        assert!(toolkit_info.activated);
    }

    // Perform a tool search for "weather" and check that one tool is returned
    let query = generator.generate_embedding_default("weather").await.unwrap();
    let tool_router = shinkai_db.get_tool_router(&profile).unwrap();


    // db.update_tool_router(&profile, |tool_router| {
    //     tool_router.add_shinkai_tool(&new_tool, /* embedding */)?;
    //     Ok(())
    // })?;

    let results = tool_router.vector_search(query, 1);
    // eprintln!("results: {:?}", results);
    for result in &results {
        eprintln!("Tool name: {}", result.name());
    }
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name(), "weather");
}
