use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tool_offering::{ToolPrice, UsageType};
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_test_framework::{run_test_one_node_network, TestConfig, TestContext};
use shinkai_tools_primitives::tools::network_tool::NetworkTool;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolWithAssets};
use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;

use super::utils::node_test_api::wait_for_default_tools;

#[test]
fn list_all_network_tools_returns_only_network_tools() {
    std::env::set_var("SKIP_IMPORT_FROM_DIRECTORY", "true");
    std::env::set_var("WELCOME_MESSAGE", "false");
    std::env::set_var("IS_TESTING", "1");

    run_test_one_node_network(TestConfig::default(), move |ctx: TestContext| {
        Box::pin(async move {
            ctx.register_device().await.unwrap();
            let tools_ready = wait_for_default_tools(ctx.commands.clone(), ctx.api_key.clone(), 60).await.unwrap();
            assert!(tools_ready);

            let provider = ShinkaiName::new(ctx.identity_name.clone()).unwrap();
            let tool_router_key = ToolRouterKey::new(
                provider.to_string(),
                ctx.identity_name.clone(),
                "echo_function".to_string(),
                None,
            );

            let network_tool = NetworkTool {
                name: "Echo Function".to_string(),
                description: "A function that returns the input string prefixed with 'echo: '.".to_string(),
                version: "1.0.0".to_string(),
                author: ctx.identity_name.clone(),
                mcp_enabled: Some(false),
                provider: provider.clone(),
                tool_router_key: tool_router_key.to_string_without_version(),
                usage_type: UsageType::PerUse(ToolPrice::Free),
                activated: true,
                config: vec![],
                input_args: Parameters::with_single_property(
                    "input",
                    "string",
                    "The input string to be echoed",
                    true,
                    None,
                ),
                output_arg: ToolOutputArg::empty(),
                embedding: None,
                restrictions: None,
            };

            let (sender, receiver) = async_channel::bounded(1);
            ctx.commands
                .send(NodeCommand::V2ApiAddShinkaiTool {
                    bearer: ctx.api_key.clone(),
                    shinkai_tool: ShinkaiToolWithAssets {
                        tool: ShinkaiTool::Network(network_tool, true),
                        assets: None,
                    },
                    res: sender,
                })
                .await
                .unwrap();
            let resp = receiver.recv().await.unwrap();
            assert!(resp.is_ok());

            let (sender, receiver) = async_channel::bounded(1);
            ctx.commands
                .send(NodeCommand::V2ApiListAllNetworkShinkaiTools {
                    bearer: ctx.api_key.clone(),
                    res: sender,
                })
                .await
                .unwrap();
            let resp = receiver.recv().await.unwrap();
            assert!(resp.is_ok());
            let tools = resp.unwrap();
            let array = tools.as_array().unwrap();
            assert_eq!(array.len(), 1);
            let tool = &array[0];
            assert_eq!(tool.get("tool_type").unwrap().as_str().unwrap(), "Network");
            assert_eq!(tool.get("name").unwrap().as_str().unwrap(), "Echo Function");

            ctx.abort_handle.abort();
        })
    });
}

