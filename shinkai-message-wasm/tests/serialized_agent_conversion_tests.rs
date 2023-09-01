use wasm_bindgen_test::*;

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_wasm::{shinkai_wasm_wrappers::serialized_agent_wrapper::SerializedAgentWrapper, schemas::agents::serialized_agent::{OpenAI, AgentAPIModel, SerializedAgent}};
    use wasm_bindgen::JsValue;
    use serde_wasm_bindgen::from_value;

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
            "openai:chatgpt3-turbo".to_string(),
            "permission1,permission2".to_string(),
            "bucket1,bucket2".to_string(),
            "sender1,sender2".to_string(),
        ).unwrap();

        // Get the inner SerializedAgent
        let agent_jsvalue = serialized_agent_wrapper.inner().unwrap();
        let agent: SerializedAgent = from_value(agent_jsvalue).unwrap();

        // Check that the fields are correctly converted
        assert_eq!(agent.id, "test_agent");
        assert_eq!(agent.full_identity_name.to_string(), "@@node.shinkai/main/agent/test_agent");
        assert_eq!(agent.perform_locally, false);
        assert_eq!(agent.external_url, Some("http://example.com".to_string()));
        assert_eq!(agent.api_key, Some("123456".to_string()));
        assert_eq!(agent.model, AgentAPIModel::OpenAI(OpenAI { model_type: "chatgpt3-turbo".to_string() }));
        assert_eq!(agent.toolkit_permissions, vec!["permission1".to_string(), "permission2".to_string()]);
        assert_eq!(agent.storage_bucket_permissions, vec!["bucket1".to_string(), "bucket2".to_string()]);
        assert_eq!(agent.allowed_message_senders, vec!["sender1".to_string(), "sender2".to_string()]);
    }
}