use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, OpenAI, SerializedLLMProvider,
};
use shinkai_test_framework::{run_test_one_node_network, TestContext, TestConfig};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use std::time::Duration;

use super::utils::node_test_api::wait_for_default_tools;
use mockito::Server;

#[test]
fn simple_job_message_test() {
    std::env::set_var("WELCOME_MESSAGE", "false");
    std::env::set_var("SKIP_IMPORT_FROM_DIRECTORY", "true");
    std::env::set_var("IS_TESTING", "1");
    let mut server = Server::new();

    {
        eprintln!("\n\nSetting up mock OpenAI server");
        let _m = server
            .mock("POST", "/v1/chat/completions")
            .match_header("authorization", "Bearer mockapikey")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                            "id": "chatcmpl-123",
                            "object": "chat.completion",
                            "created": 1677652288,
                            "choices": [{
                                "index": 0,
                                "message": {
                                    "role": "assistant",
                                    "content": "This is a test response from the mock server"
                                },
                                "finish_reason": "stop"
                            }],
                            "usage": {
                                "prompt_tokens": 9,
                                "completion_tokens": 12,
                                "total_tokens": 21
                            }
                        }"#,
            )
            .create();
    }

    {
        let _m = server
            .mock("POST", "/api/embeddings")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{\"embedding\": [0.0,0.0,0.0]}")
            .create();
    }

    let server_url = server.url();

    let config = TestConfig::default()
        .with_mock_openai(server_url.clone())
        .with_mock_embeddings(server_url.clone());

    run_test_one_node_network(config, move |ctx: TestContext| Box::pin(async move {

        ctx.register_device().await.unwrap();
    let tools_ready = wait_for_default_tools(ctx.commands.clone(), ctx.api_key.clone(), 120)
        .await
        .unwrap();
    assert!(tools_ready);

    let agent_name = ShinkaiName::new(
        format!("{}/{}/agent/test_agent", ctx.identity_name, ctx.profile_name).to_string(),
    )
    .unwrap();

    let open_ai = OpenAI { model_type: "gpt-4-turbo".to_string() };
    let agent = SerializedLLMProvider {
        id: "test_agent".to_string(),
        full_identity_name: agent_name,
        name: Some("Test Agent".to_string()),
        description: Some("Test Agent Description".to_string()),
        external_url: Some(server_url.clone()),
        api_key: Some("mockapikey".to_string()),
        model: LLMProviderInterface::OpenAI(open_ai),
    };

        ctx.register_llm_provider(agent).await.unwrap();

    let agent_sub = format!("{}/agent/test_agent", ctx.profile_name);
        let job_id = ctx.create_job(&agent_sub).await.unwrap();

        ctx
            .update_job_config(
                &job_id,
                JobConfig {
                    stream: Some(false),
                    ..JobConfig::empty()
                },
            )
            .await
            .unwrap();

        ctx.send_job_message(&job_id, "This is a test message").await.unwrap();

        let response = ctx
            .wait_for_response(Duration::from_secs(10))
            .await
            .unwrap();
        assert!(response.contains("This is a test response from the mock server"));

        ctx.abort_handle.abort();
    }));
}
