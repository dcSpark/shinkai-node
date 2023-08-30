use serde_json::Value as JsonValue;
use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::tools::js_toolkit::JSToolkit;
use shinkai_node::tools::js_toolkit_executor::JSToolkitExecutor;
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

fn load_test_js_toolkit_from_file() -> Result<String, std::io::Error> {
    let path = "./files/packaged-shinkai-toolkit.js";
    let data = std::fs::read_to_string(path)?;
    Ok(data)
}

#[test]
fn test_default_js_toolkit_json_parsing() {
    let toolkit = JSToolkit::from_toolkit_json(&default_toolkit_json(), "").unwrap();

    assert_eq!(toolkit.name, "Google Calendar Toolkit");
    assert_eq!(
        toolkit.tools[0].ebnf_inputs(false).replace("\n", ""),
        r#"{"calendar_id": calendar_id, "text": text, "send_updates": send_updates, "toolkit_name": Google Calendar Toolkit, }calendar_id :== ([a-zA-Z0-9_]+)?text :== ([a-zA-Z0-9_]+)send_updates :== ("all" | "externalOnly" | "none")?"#
    );

    assert_eq!(toolkit.header_definitions.len(), 4);
    assert_eq!(toolkit.version, "0.0.1".to_string());
    assert_eq!(toolkit.author, "Shinkai Team".to_string());
}

#[test]
fn test_js_toolkit_execution_and_installing() {
    // Load the toolkit
    let toolkit_js_code = load_test_js_toolkit_from_file().unwrap();

    // Create the executor
    let executor = JSToolkitExecutor::new_local().unwrap();

    // Test submit_toolkit_json_request
    let toolkit = executor.submit_toolkit_json_request(&toolkit_js_code).unwrap();
    assert_eq!(&toolkit.name, "toolkit-example");
    assert_eq!(toolkit.tools.len(), 2);

    // Test submit_headers_validation_request
    let header_values = HashMap::new();
    let headers_validation_result = executor
        .submit_headers_validation_request(&toolkit_js_code, &header_values)
        .unwrap();
    assert_eq!(headers_validation_result, true);

    // Test submit_tool_execution_request
    let tool = "isEven";
    let input_data = &serde_json::json!({"number": 56});
    let tool_execution_result = executor
        .submit_tool_execution_request(tool, input_data, &toolkit_js_code, &header_values)
        .unwrap();

    assert_eq!(tool_execution_result.result[0].output.as_bool().unwrap(), true);
    assert_eq!(tool_execution_result.tool, "isEven");

    // Install the toolkit
    let db_path = format!("db_tests/{}", "embeddings");
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    let profile = default_test_profile();
    shinkai_db.init_profile_tool_structs(&profile).unwrap();
    shinkai_db.install_toolkit(&toolkit, &profile).unwrap();
    assert!(shinkai_db.check_if_toolkit_installed(&toolkit, &profile).unwrap());

    // Uninstall and check via the toolkit map and db key (TODO: later add deactivation checks too)
    shinkai_db.uninstall_toolkit(&toolkit.name, &profile).unwrap();
    assert!(shinkai_db.check_if_toolkit_installed(&toolkit, &profile).unwrap() == false);
    let fetched_toolkit = shinkai_db.get_toolkit(&toolkit.name, &profile);
    assert!(fetched_toolkit.is_err());
}
