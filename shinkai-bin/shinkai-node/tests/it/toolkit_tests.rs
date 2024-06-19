use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::tools::js_toolkit::JSToolkit;
use shinkai_node::tools::js_toolkit_executor::JSToolkitExecutor;
use shinkai_node::tools::router::ShinkaiTool;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

fn default_test_profile() -> ShinkaiName {
    ShinkaiName::new("@@alice.shinkai/profileName".to_string()).unwrap()
}

fn default_toolkit_json() -> JsonValue {
    let json_string = r#"{"toolkitName":"Google Calendar Toolkit", "author":"Shinkai Team","version":"0.0.1","toolkitHeaders":[{"name":"OAUTH","oauth":{"description":"","displayName":"Authentication","authUrl":"https://accounts.google.com/o/oauth2/auth","tokenUrl":"https://oauth2.googleapis.com/token","required":true,"pkce":true,"scope":["https://www.googleapis.com/auth/calendar.events","https://www.googleapis.com/auth/calendar.readonly"],"cloudOAuth":"activepieces"},"header":"x-shinkai-oauth"},{"name":"API_KEY","description":"Some Optional API Key","type":"STRING","isOptional":true,"header":"x-shinkai-api-key"},{"name":"API_SECRET","description":"Api Secret key","type":"STRING","header":"x-shinkai-api-secret"},{"name":"BASE_URL","description":"Base URL for api","type":"STRING","header":"x-shinkai-base-url"}],"tools":[{"name":"GoogleCalendarQuickEvent","description":"Activepieces Create Quick Event at Google Calendar","input":[{"name":"calendar_id","type":"STRING","description":"Primary calendar used if not specified","isOptional":true,"wrapperType":"none","ebnf":"([a-zA-Z0-9_]+)?"},{"name":"text","type":"STRING","description":"The text describing the event to be created","isOptional":false,"wrapperType":"none","ebnf":"([a-zA-Z0-9_]+)"},{"name":"send_updates","type":"ENUM","description":"Guests who should receive notifications about the creation of the new event.","isOptional":true,"wrapperType":"none","enum":["all","externalOnly","none"],"ebnf":"(\"all\" | \"externalOnly\" | \"none\")?"}],"output":[{"name":"response","type":"STRING","description":"Network Response","isOptional":false,"wrapperType":"none","ebnf":"([a-zA-Z0-9_]+)"}],"inputEBNF":"calendar_id ::= ([a-zA-Z0-9_]+)?\ntext ::= ([a-zA-Z0-9_]+)\nsend_updates ::= (\"all\" | \"externalOnly\" | \"none\")?\nresponse ::= ([a-zA-Z0-9_]+)"}]}"#.to_string();
    let parsed_json: JsonValue = serde_json::from_str(&json_string).unwrap();
    parsed_json
}

async fn default_toolkit_header_values() -> Result<JsonValue, Box<dyn std::error::Error>> {
    // Ok(JsonValue::Null)
    let path = "./files/example-toolkit-setup.json";
    let data = tokio::fs::read_to_string(path).await?;
    let header_values = serde_json::from_str(&data).unwrap_or(JsonValue::Null);
    Ok(header_values)
}

fn load_test_js_toolkit_from_file() -> Result<String, std::io::Error> {
    let path = "./files/example-packaged-shinkai-toolkit.js";
    let data = std::fs::read_to_string(path)?;
    Ok(data)
}

// #[test]
// fn test_default_js_toolkit_json_parsing() {
//     init_default_tracing(); 
//     let toolkit = JSToolkit::from_toolkit_json(&default_toolkit_json(), "").unwrap();

//     assert_eq!(toolkit.name, "Google Calendar Toolkit");
//     assert_eq!(
//         ShinkaiTool::from(toolkit.tools[0].clone())
//             .replace("\n", ""),
//         r#"{"calendar_id": calendar_id, "text": text, "send_updates": send_updates, "toolkit": Google Calendar Toolkit, }calendar_id :== ([a-zA-Z0-9_]+)?text :== ([a-zA-Z0-9_]+)send_updates :== ("all" | "externalOnly" | "none")?"#
//     );

//     assert_eq!(toolkit.header_definitions.len(), 4);
//     assert_eq!(toolkit.version, "0.0.1".to_string());
//     assert_eq!(toolkit.author, "Shinkai Team".to_string());
// }

// #[tokio::test]
async fn test_js_toolkit_execution() {
    init_default_tracing(); 
    setup();
    // Load the toolkit
    let toolkit_js_code = load_test_js_toolkit_from_file().unwrap();

    // Create the executor
    let executor = JSToolkitExecutor::new_local().await.unwrap();

    // Test submit_toolkit_json_request
    let toolkit = executor.submit_toolkit_json_request(&toolkit_js_code).await.unwrap();
    assert_eq!(&toolkit.name, "@shinkai_network/toolkit-example");
    assert_eq!(toolkit.tools.len(), 2);

    // Test submit_headers_validation_request
    let header_values = &default_toolkit_header_values().await.unwrap();
    let headers_validation_result = executor
        .submit_headers_validation_request(&toolkit_js_code, &header_values)
        .await
        .unwrap();
    // Test submit_tool_execution_request
    let tool = "isEven";
    let input_data = &serde_json::json!({"number": 56});
    let tool_execution_result = executor
        .submit_tool_execution_request(tool, input_data, &toolkit_js_code, &header_values)
        .await
        .unwrap();

    assert_eq!(tool_execution_result.result[0].output.as_bool().unwrap(), true);
    assert_eq!(tool_execution_result.tool, "isEven");
}

// #[tokio::test]
async fn test_toolkit_installation_and_retrieval() {
    init_default_tracing(); 
    setup();
    // Load the toolkit
    let toolkit_js_code = load_test_js_toolkit_from_file().unwrap();

    // Create the executor
    let executor = JSToolkitExecutor::new_local().await.unwrap();

    // Test submit_toolkit_json_request
    let toolkit = executor.submit_toolkit_json_request(&toolkit_js_code).await.unwrap();

    // Install the toolkit
    let db_path = format!("db_tests/{}", "toolkit");
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    let profile = default_test_profile();
    shinkai_db.init_profile_tool_structs(&profile).unwrap();
    shinkai_db.install_toolkit(&toolkit, &profile).unwrap();
    assert!(shinkai_db.check_if_toolkit_installed(&toolkit, &profile).unwrap());

    // Assert that the retrieved toolkit is equivalent to the original one
    let retrieved_toolkit = shinkai_db.get_toolkit(&toolkit.name, &profile).unwrap();
    assert_eq!(toolkit, retrieved_toolkit);

    // Uninstall and check via the toolkit map and db key (TODO: later add deactivation checks too)
    shinkai_db.uninstall_toolkit(&toolkit.name, &profile).unwrap();
    assert!(shinkai_db.check_if_toolkit_installed(&toolkit, &profile).unwrap() == false);
    let fetched_toolkit = shinkai_db.get_toolkit(&toolkit.name, &profile);
    assert!(fetched_toolkit.is_err());
}

// #[tokio::test]
async fn test_tool_router_and_toolkit_flow() {
    init_default_tracing(); 
    setup();

    let generator = RemoteEmbeddingGenerator::new_default();

    // Load the toolkit
    let toolkit_js_code = load_test_js_toolkit_from_file().unwrap();

    // Create the executor
    let executor = JSToolkitExecutor::new_local().await.unwrap();

    // Test submit_toolkit_json_request
    let toolkit = executor.submit_toolkit_json_request(&toolkit_js_code).await.unwrap();

    // Install the toolkit
    let db_path = format!("db_tests/{}", "toolkit");
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    let profile = default_test_profile();
    shinkai_db.init_profile_tool_structs(&profile).unwrap();
    shinkai_db.install_toolkit(&toolkit, &profile).unwrap();
    assert!(shinkai_db.check_if_toolkit_installed(&toolkit, &profile).unwrap());

    // Set headers and activate the toolkit to add it to the tool router
    shinkai_db
        .set_toolkit_header_values(
            &toolkit.name,
            &profile,
            &default_toolkit_header_values().await.unwrap(),
            &executor,
        )
        .await
        .unwrap();
    println!("passed setting");
    shinkai_db
        .activate_toolkit(&toolkit.name, &profile, &executor, Box::new(generator.clone()))
        .await
        .unwrap();
    println!("passed activating");

    // Retrieve the tool router
    let tool_router = shinkai_db.get_tool_router(&profile).unwrap();
    println!("passed tool router");

    // Vector Search
    let query = generator
        .generate_embedding_default("Is 25 an odd or even number?")
        .await
        .unwrap();
    let results1 = tool_router.vector_search(query, 10);
    assert_eq!(results1[0].name(), "isEven");

    let query = generator
        .generate_embedding_default("I want to multiply 500 x 1523 and see if it is greater than 50000")
        .await
        .unwrap();
    let results2 = tool_router.vector_search(query, 1);
    assert_eq!(results2[0].name(), "CompareNumbers");

    let query = generator
        .generate_embedding_default(
            "Send a message to @@alice.shinkai asking her what the status is on the project estimates.",
        )
        .await
        .unwrap();
    let results3 = tool_router.vector_search(query, 10);
    assert_eq!(results3[0].name(), "Send_Message");

    let query = generator
        .generate_embedding_default(
            "Search through my documents and find the pdf with the March company financial report.",
        )
        .await
        .unwrap();
    let results4 = tool_router.vector_search(query, 10);
    // assert_eq!(results4[0].name(), "User_Data_Vector_Search");

    // Deactivate toolkit and check to make sure tools are removed from Tool Router
    shinkai_db.deactivate_toolkit(&toolkit.name, &profile).unwrap();
    let tool_router = shinkai_db.get_tool_router(&profile).unwrap();
    assert!(tool_router
        .get_shinkai_tool(&results1[0].toolkit_type_name(), &results1[0].name())
        .is_err());
    assert!(tool_router
        .get_shinkai_tool(&results2[0].toolkit_type_name(), &results2[0].name())
        .is_err());

    // Check toolkit is still installed, then uninstall, and check again
    assert!(shinkai_db.check_if_toolkit_installed(&toolkit, &profile).unwrap());
    shinkai_db.uninstall_toolkit(&toolkit.name, &profile).unwrap();
    assert!(!shinkai_db.check_if_toolkit_installed(&toolkit, &profile).unwrap());
}

// A fake test which purposefully fails so that we can generate embeddings
// for all existing rust tools and print them into console (so we can copy-paste)
// and hard-code them in rust_tools.rs.
// Temporary solution
// #[test]
// fn generate_rust_tool_embeddings() {
//     setup();
//
//     let generator = RemoteEmbeddingGenerator::new_default();

//     for t in RUST_TOOLKIT.rust_tool_map.values() {
//         let tool = ShinkaiTool::Rust(t.clone());
//         let embedding = generator.generate_embedding_default(&tool.format_embedding_string()).await.unwrap();

//         println!("{}\n{:?}\n\n", tool.name(), embedding.vector)
//     }

//     assert_eq!(1, 2);
// }
