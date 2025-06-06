use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_tools_primitives::tools::deno_tools::DenoTool;
use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolWithAssets};

use super::utils::node_test_api::{api_initial_registration_with_no_code_for_device, wait_for_default_tools};
use utils::test_boilerplate::run_test_one_node_network;

fn get_echo_tool_json_string() -> String {
    r#"{
        \"content\": [
            {
                \"activated\": false,
                \"assets\": [],
                \"author\": \"@@localhost.sep-shinkai\",
                \"config\": [],
                \"configFormData\": {},
                \"configurations\": { \"properties\": {}, \"required\": [], \"type\": \"object\" },
                \"description\": \"A function that echoes back the input message.\",
                \"embedding\": null,
                \"file_inbox\": null,
                \"homepage\": null,
                \"input_args\": {
                    \"properties\": { \"message\": { \"description\": \"The message to echo\", \"type\": \"string\" } },
                    \"required\": [\"message\"],
                    \"type\": \"object\"
                },
                \"js_code\": \"type CONFIG = {};\\ntype INPUTS = { message: string };\\ntype OUTPUT = { echoed: string };\\n\\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\\n    return { echoed: inputs.message };\\n}\",
                \"keywords\": [\"echo\", \"message\", \"repeat\"],
                \"mcp_enabled\": false,
                \"name\": \"Echo Function\",
                \"oauth\": [],
                \"operating_system\": [\"linux\", \"macos\", \"windows\"],
                \"output_arg\": { \"json\": \"{}\" },
                \"result\": {
                    \"properties\": { \"echoed\": { \"description\": \"The echoed message\", \"type\": \"string\" } },
                    \"required\": [\"echoed\"],
                    \"type\": \"object\"
                },
                \"runner\": \"any\",
                \"sql_queries\": [],
                \"sql_tables\": [],
                \"tool_router_key\": \"local:::__localhost_sep_shinkai:::echo_function\",
                \"tool_set\": \"\",
                \"tools\": [],
                \"version\": \"1.0.0\"
            },
            true
        ],
        \"type\": \"Deno\"
    }"#.to_string()
}

#[test]
fn add_and_get_echo_tool_router_key() {
    std::env::set_var("WELCOME_MESSAGE", "false");
    std::env::set_var("SKIP_IMPORT_FROM_DIRECTORY", "true");
    std::env::set_var("IS_TESTING", "1");

    run_test_one_node_network(|env| {
        Box::pin(async move {
            let sender = env.node1_commands_sender.clone();
            let node1_encryption_pk = env.node1_encryption_pk;
            let device_sk = env.node1_device_encryption_sk.clone();
            let profile_sk = env.node1_profile_encryption_sk.clone();
            let device_sig_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let profile_sig_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
            let api_key = env.node1_api_key.clone();
            let device_name = env.node1_device_name.clone();

            api_initial_registration_with_no_code_for_device(
                sender.clone(),
                env.node1_profile_name.as_str(),
                env.node1_identity_name.as_str(),
                node1_encryption_pk,
                device_sk.clone(),
                device_sig_sk,
                profile_sk.clone(),
                profile_sig_sk,
                device_name.as_str(),
            )
            .await;

            wait_for_default_tools(sender.clone(), api_key.clone(), 20)
                .await
                .unwrap();

            let tool_json = get_echo_tool_json_string();
            let parsed: serde_json::Value = serde_json::from_str(&tool_json).unwrap();
            let deno_value = parsed["content"][0].clone();
            let active = parsed["content"][1].as_bool().unwrap();
            let deno_tool: DenoTool = serde_json::from_value(deno_value).unwrap();
            let shinkai_tool = ShinkaiTool::Deno(deno_tool.clone(), active);

            let (res_tx, res_rx) = async_channel::bounded(1);
            sender
                .send(NodeCommand::V2ApiAddShinkaiTool {
                    bearer: api_key.clone(),
                    shinkai_tool: ShinkaiToolWithAssets {
                        tool: shinkai_tool,
                        assets: None,
                    },
                    res: res_tx,
                })
                .await
                .unwrap();
            let add_resp = res_rx.recv().await.unwrap();
            assert!(add_resp.is_ok(), "Failed to add tool: {:?}", add_resp);

            let key = deno_tool.tool_router_key.as_ref().unwrap().to_string_without_version();

            let (res_tx, res_rx) = async_channel::bounded(1);
            sender
                .send(NodeCommand::V2ApiGetShinkaiTool {
                    bearer: api_key.clone(),
                    payload: key.clone(),
                    serialize_config: false,
                    res: res_tx,
                })
                .await
                .unwrap();
            let resp = res_rx.recv().await.unwrap().expect("Get tool failed");
            let fetched: ShinkaiTool = serde_json::from_value(resp).unwrap();
            assert_eq!(fetched.tool_router_key().to_string_without_version(), key);

            env.node1_abort_handler.abort();
        })
    });
}
