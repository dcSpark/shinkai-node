use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_node::db::ShinkaiDB;
use shinkai_tools_runner::built_in_tools;
use shinkai_node::tools::js_toolkit::JSToolkit;
use shinkai_node::tools::js_toolkit_executor::JSToolkitExecutor;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;

use crate::it::utils::db_handlers::setup;

fn default_test_profile() -> ShinkaiName {
    ShinkaiName::new("@@alice.shinkai/profileName".to_string()).unwrap()
}

#[tokio::test]
async fn test_toolkit_installation_from_built_in_tools() {
    init_default_tracing();
    setup();

    // // Create the executor
    // let executor = JSToolkitExecutor::new_local().await.unwrap();

    // Initialize the database and profile
    let db_path = format!("db_tests/{}", "toolkit");
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    let profile = default_test_profile();
    let generator = RemoteEmbeddingGenerator::new_default();
    shinkai_db
        .init_profile_tool_structs(&profile, Box::new(generator))
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
    assert_eq!(toolkit_map.get_all_toolkit_infos().len(), 4);
}