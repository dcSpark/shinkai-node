use wasm_bindgen_test::*;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_wasm_bindgen::from_value;
    use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
        LLMProviderInterface, OpenAI, SerializedLLMProvider,
    };
    use shinkai_message_wasm::shinkai_wasm_wrappers::serialized_llm_provider_wrapper::SerializedLLMProviderWrapper;
    use wasm_bindgen::JsValue;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_conversion_from_wasm_serialized_agent_to_serialized_agent() {
        // Create a SerializedLLMProviderWrapper using from_strings
        let serialized_llm_provider_wrapper = SerializedLLMProviderWrapper::from_strings(
            "test_agent".to_string(),
            "@@node.shinkai/main/agent/test_agent".to_string(),
            "http://example.com".to_string(),
            "123456".to_string(),
            "openai:gpt-3.5-turbo-1106".to_string(),
        )
        .unwrap();

        // Get the inner SerializedLLMProvider
        let agent_jsvalue = serialized_llm_provider_wrapper.inner().unwrap();
        let agent: SerializedLLMProvider = from_value(agent_jsvalue).unwrap();

        // Check that the fields are correctly converted
        assert_eq!(agent.id, "test_agent");
        assert_eq!(
            agent.full_identity_name.to_string(),
            "@@node.shinkai/main/agent/test_agent"
        );
        assert_eq!(agent.external_url, Some("http://example.com".to_string()));
        assert_eq!(agent.api_key, Some("123456".to_string()));
        assert_eq!(
            agent.model,
            LLMProviderInterface::OpenAI(OpenAI {
                model_type: "gpt-3.5-turbo-1106".to_string()
            })
        );
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_serialization_and_deserialization_of_serialized_llm_provider_wrapper() {
        // console_log::init_with_level(log::Level::Debug).expect("error initializing log");
        // Create a SerializedLLMProviderWrapper using from_strings
        let serialized_llm_provider_wrapper = SerializedLLMProviderWrapper::from_strings(
            "test_agent".to_string(),
            "@@node.shinkai/main/agent/test_agent".to_string(),
            "http://example.com".to_string(),
            "123456".to_string(),
            "openai:gpt-3.5-turbo-1106".to_string(),
        )
        .unwrap();

        // Serialize the SerializedLLMProviderWrapper to a JSON string
        let serialized_llm_provider_wrapper_json = serialized_llm_provider_wrapper.to_json_str().unwrap();
        // log::debug!("serialized agent: {}", serialized_llm_provider_wrapper_json);

        assert_eq!(
            serialized_llm_provider_wrapper_json.contains("\"model\":\"openai:gpt-3.5-turbo-1106\""),
            true
        );

        // Deserialize the JSON string back to a SerializedLLMProviderWrapper
        let deserialized_llm_provider_wrapper =
            SerializedLLMProviderWrapper::from_json_str(&serialized_llm_provider_wrapper_json).unwrap();
        // log::debug!("deserialized agent: {:?}", deserialized_llm_provider_wrapper);

        // Check that the fields are correctly converted
        let agent = deserialized_llm_provider_wrapper.inner().unwrap();
        let agent: SerializedLLMProvider = from_value(agent).unwrap();
        assert_eq!(agent.id, "test_agent");
        assert_eq!(
            agent.full_identity_name.to_string(),
            "@@node.shinkai/main/agent/test_agent"
        );
        assert_eq!(agent.external_url, Some("http://example.com".to_string()));
        assert_eq!(agent.api_key, Some("123456".to_string()));
        assert_eq!(
            agent.model,
            LLMProviderInterface::OpenAI(OpenAI {
                model_type: "gpt-3.5-turbo-1106".to_string()
            })
        );
    }
}
