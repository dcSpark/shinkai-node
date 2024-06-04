use wasm_bindgen_test::*;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_wasm_bindgen::from_value;
    use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, OpenAI, SerializedAgent};
    use shinkai_message_wasm::shinkai_wasm_wrappers::serialized_agent_wrapper::SerializedAgentWrapper;
    use wasm_bindgen::JsValue;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_conversion_from_wasm_serialized_agent_to_serialized_agent() {
        // Create a SerializedAgentWrapper using from_strings
        let serialized_agent_wrapper = SerializedAgentWrapper::from_strings(
            "test_agent".to_string(),
            "@@node.shinkai/main/agent/test_agent".to_string(),
            "false".to_string(),
            "http://example.com".to_string(),
            "123456".to_string(),
            "openai:gpt-3.5-turbo-1106".to_string(),
            "permission1,permission2".to_string(),
            "bucket1,bucket2".to_string(),
            "sender1,sender2".to_string(),
        )
        .unwrap();

        // Get the inner SerializedAgent
        let agent_jsvalue = serialized_agent_wrapper.inner().unwrap();
        let agent: SerializedAgent = from_value(agent_jsvalue).unwrap();

        // Check that the fields are correctly converted
        assert_eq!(agent.id, "test_agent");
        assert_eq!(
            agent.full_identity_name.to_string(),
            "@@node.shinkai/main/agent/test_agent"
        );
        assert_eq!(agent.perform_locally, false);
        assert_eq!(agent.external_url, Some("http://example.com".to_string()));
        assert_eq!(agent.api_key, Some("123456".to_string()));
        assert_eq!(
            agent.model,
            AgentLLMInterface::OpenAI(OpenAI {
                model_type: "gpt-3.5-turbo-1106".to_string()
            })
        );
        assert_eq!(
            agent.toolkit_permissions,
            vec!["permission1".to_string(), "permission2".to_string()]
        );
        assert_eq!(
            agent.storage_bucket_permissions,
            vec!["bucket1".to_string(), "bucket2".to_string()]
        );
        assert_eq!(
            agent.allowed_message_senders,
            vec!["sender1".to_string(), "sender2".to_string()]
        );
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_serialization_and_deserialization_of_serialized_agent_wrapper() {
        // console_log::init_with_level(log::Level::Debug).expect("error initializing log");
        // Create a SerializedAgentWrapper using from_strings
        let serialized_agent_wrapper = SerializedAgentWrapper::from_strings(
            "test_agent".to_string(),
            "@@node.shinkai/main/agent/test_agent".to_string(),
            "false".to_string(),
            "http://example.com".to_string(),
            "123456".to_string(),
            "openai:gpt-3.5-turbo-1106".to_string(),
            "permission1,permission2".to_string(),
            "bucket1,bucket2".to_string(),
            "sender1,sender2".to_string(),
        )
        .unwrap();

        // Serialize the SerializedAgentWrapper to a JSON string
        let serialized_agent_wrapper_json = serialized_agent_wrapper.to_json_str().unwrap();
        // log::debug!("serialized agent: {}", serialized_agent_wrapper_json);

        assert_eq!(
            serialized_agent_wrapper_json.contains("\"model\":\"openai:gpt-3.5-turbo-1106\""),
            true
        );

        // Deserialize the JSON string back to a SerializedAgentWrapper
        let deserialized_agent_wrapper = SerializedAgentWrapper::from_json_str(&serialized_agent_wrapper_json).unwrap();
        // log::debug!("deserialized agent: {:?}", deserialized_agent_wrapper);

        // Check that the fields are correctly converted
        let agent = deserialized_agent_wrapper.inner().unwrap();
        let agent: SerializedAgent = from_value(agent).unwrap();
        assert_eq!(agent.id, "test_agent");
        assert_eq!(
            agent.full_identity_name.to_string(),
            "@@node.shinkai/main/agent/test_agent"
        );
        assert_eq!(agent.perform_locally, false);
        assert_eq!(agent.external_url, Some("http://example.com".to_string()));
        assert_eq!(agent.api_key, Some("123456".to_string()));
        assert_eq!(
            agent.model,
            AgentLLMInterface::OpenAI(OpenAI {
                model_type: "gpt-3.5-turbo-1106".to_string()
            })
        );
        assert_eq!(
            agent.toolkit_permissions,
            vec!["permission1".to_string(), "permission2".to_string()]
        );
        assert_eq!(
            agent.storage_bucket_permissions,
            vec!["bucket1".to_string(), "bucket2".to_string()]
        );
        assert_eq!(
            agent.allowed_message_senders,
            vec!["sender1".to_string(), "sender2".to_string()]
        );
    }
}
