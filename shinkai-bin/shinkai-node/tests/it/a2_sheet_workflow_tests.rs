use super::utils::test_boilerplate::run_test_one_node_network;
use shinkai_dsl::parser::parse_workflow;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, OpenAI, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_sheet::sheet::{ColumnBehavior, ColumnDefinition};

use super::utils::node_test_api::{
    api_get_all_smart_inboxes_from_profile, api_initial_registration_with_no_code_for_device,
    api_llm_provider_registration,
};
use mockito::Server;

#[test]
fn create_a_sheet_and_check_workflows() {
    unsafe { std::env::set_var("WELCOME_MESSAGE", "false") };
    init_default_tracing();
    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_agent = env.node1_llm_provider.clone();
            let node1_encryption_pk = env.node1_encryption_pk;
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
            let node1_abort_handler = env.node1_abort_handler;
            let node1_sheet_manager = env.node1_sheet_manager.clone();

            {
                // Register a Profile in Node1 and verifies it
                eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
                api_initial_registration_with_no_code_for_device(
                    node1_commands_sender.clone(),
                    env.node1_profile_name.as_str(),
                    env.node1_identity_name.as_str(),
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_device_identity_sk),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_device_name.as_str(),
                )
                .await;
            }
            let mut server = Server::new();
            {
                // Register an Agent
                eprintln!("\n\nRegister an Agent in Node1 and verify it");
                let agent_name = ShinkaiName::new(
                    format!(
                        "{}/{}/agent/{}",
                        node1_identity_name.clone(),
                        node1_profile_name.clone(),
                        node1_agent.clone()
                    )
                    .to_string(),
                )
                .unwrap();

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
                            "content": "Hola Mundo"
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

                let open_ai = OpenAI {
                    model_type: "gpt-4o-mini".to_string(),
                };

                let agent = SerializedLLMProvider {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    perform_locally: false,
                    external_url: Some(server.url()),
                    api_key: Some("mockapikey".to_string()),
                    model: LLMProviderInterface::OpenAI(open_ai),
                    toolkit_permissions: vec![],
                    storage_bucket_permissions: vec![],
                    allowed_message_senders: vec![],
                };
                api_llm_provider_registration(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    agent,
                )
                .await;
            }

            // Create a Sheet that looks like this
            // Row | Column A     | Column B                   | Column C
            //     | (Text)       | (LLMCall)                  | (Formula)
            //     |              | input: "=A"                | "=B + " And Space""
            //     |              | workflow: WorkflowTest     |
            //     |              | llm_provider: node1_agent  |
            // --------------------------------------------------------------------
            //   0 | Hello        | Hola Mundo                 | Hola Mundo And Space

            let mut sheet_id = "".to_string();
            {
                let sheet_manager = node1_sheet_manager.clone();
                let mut sheet_manager = sheet_manager.lock().await;

                // Create a new empty sheet
                sheet_manager.create_empty_sheet().unwrap();

                // Get the ID of the newly created sheet
                let sheets = sheet_manager.get_user_sheets().await.unwrap();
                sheet_id = sheets.last().unwrap().uuid.clone();

                // Define columns
                let column_text = ColumnDefinition {
                    id: 0,
                    name: "Column A".to_string(),
                    behavior: ColumnBehavior::Text,
                };

                let workflow_str = r#"
                workflow WorkflowTest v0.1 {
                    step Main {
                        $RESULT = call opinionated_inference($INPUT)
                    }
                }
                "#;
                let workflow = parse_workflow(workflow_str).unwrap();

                let column_llm = ColumnDefinition {
                    id: 1,
                    name: "Column B".to_string(),
                    behavior: ColumnBehavior::LLMCall {
                        input: "=A".to_string(),
                        workflow,
                        llm_provider_name: node1_agent.clone(),
                        input_hash: None,
                    },
                };

                let column_formula = ColumnDefinition {
                    id: 2,
                    name: "Column C".to_string(),
                    behavior: ColumnBehavior::Formula("=B + \" And Space\"".to_string()),
                };

                // Set columns
                sheet_manager.set_column(&sheet_id, column_text).await.unwrap();
                sheet_manager.set_column(&sheet_id, column_llm).await.unwrap();
                sheet_manager.set_column(&sheet_id, column_formula).await.unwrap();

                // Set value in Column A
                sheet_manager
                    .set_cell_value(&sheet_id, 0, 0, "Hello".to_string())
                    .await
                    .unwrap();
            }
            {
                // api_get_all_smart_inboxes_from_profile
                eprintln!("\n\n Get All Smart Inboxes");
                let inboxes = api_get_all_smart_inboxes_from_profile(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    node1_identity_name.clone().as_str(),
                )
                .await;
                eprintln!("inboxes: {:?}", inboxes);
                assert_eq!(inboxes.len(), 1);
            }
            node1_abort_handler.abort();
        })
    });
}
