#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
        Claude, DeepSeek, Gemini, Grok, LLMProviderInterface, OpenAI, SerializedLLMProvider,
    };
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

    use shinkai_node::managers::model_capabilities_manager::{
        ModelCapabilitiesManager, ModelCapability, ModelCost, ModelPrivacy,
    };
    use std::env;
    use std::path::PathBuf;
    use std::sync::Arc;

    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use shinkai_sqlite::SqliteManager;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type = EmbeddingModelType::default();

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[tokio::test]
    async fn test_has_capability() {
        let db = setup_test_db();
        let db = Arc::new(db);
        let db_weak = Arc::downgrade(&db);

        let llm_provider_id = "agent_id1".to_string();
        let llm_provider_name =
            ShinkaiName::new(format!("@@localhost.shinkai/main/agent/{}", llm_provider_id.clone()).to_string())
                .unwrap();

        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo-1106".to_string(),
        };

        let gpt_3_5_llm_provider = SerializedLLMProvider {
            id: llm_provider_id.clone(),
            name: Some("Test Agent".to_string()),
            description: Some("Test Agent Description".to_string()),
            full_identity_name: llm_provider_name,
            external_url: Some("https://api.openai.com".to_string()),
            api_key: env::var("INITIAL_AGENT_API_KEY").ok(),
            model: LLMProviderInterface::OpenAI(open_ai),
        };

        let manager = ModelCapabilitiesManager {
            db: db_weak,
            profile: ShinkaiName::new("@@localhost.shinkai/test_profile".to_string()).unwrap(),
            llm_providers: vec![gpt_3_5_llm_provider.clone()],
        };

        assert!(manager.has_capability(ModelCapability::TextInference).await);
        assert!(!manager.has_capability(ModelCapability::ImageAnalysis).await);

        let capabilities = ModelCapabilitiesManager::get_capability(&gpt_3_5_llm_provider);
        assert_eq!(capabilities.0, vec![ModelCapability::TextInference]);
        assert_eq!(capabilities.1, ModelCost::VeryCheap);
        assert_eq!(capabilities.2, ModelPrivacy::RemoteGreedy);
    }

    #[tokio::test]
    async fn test_gpt_4_vision_preview_capabilities() {
        let db = setup_test_db();
        let db = Arc::new(db);
        let db_weak = Arc::downgrade(&db);

        let llm_provider_id = "agent_id2".to_string();
        let llm_provider_name =
            ShinkaiName::new(format!("@@localhost.shinkai/main/agent/{}", llm_provider_id.clone()).to_string())
                .unwrap();

        let open_ai = OpenAI {
            model_type: "gpt-4-vision-preview".to_string(),
        };

        let gpt_4_vision_llm_provider = SerializedLLMProvider {
            id: llm_provider_id.clone(),
            name: Some("Test Agent".to_string()),
            description: Some("Test Agent Description".to_string()),
            full_identity_name: llm_provider_name,
            external_url: Some("https://api.openai.com".to_string()),
            api_key: env::var("INITIAL_AGENT_API_KEY").ok(),
            model: LLMProviderInterface::OpenAI(open_ai),
        };

        let manager = ModelCapabilitiesManager {
            db: db_weak,
            profile: ShinkaiName::new("@@localhost.shinkai/test_profile".to_string()).unwrap(),
            llm_providers: vec![gpt_4_vision_llm_provider],
        };

        assert!(manager.has_capability(ModelCapability::TextInference).await);
        assert!(manager.has_capability(ModelCapability::ImageAnalysis).await);
        assert!(!manager.has_capability(ModelCapability::ImageGeneration).await);
    }

    #[tokio::test]
    async fn test_fake_gpt_model_capabilities() {
        let db = setup_test_db();
        let db = Arc::new(db);
        let db_weak = Arc::downgrade(&db);

        let agent_id = "agent_id3".to_string();
        let agent_name =
            ShinkaiName::new(format!("@@localhost.shinkai/main/agent/{}", agent_id.clone()).to_string()).unwrap();

        let open_ai = OpenAI {
            model_type: "gpt-fake-model".to_string(),
        };

        let fake_gpt_agent = SerializedLLMProvider {
            id: agent_id.clone(),
            name: Some("Test Agent".to_string()),
            description: Some("Test Agent Description".to_string()),
            full_identity_name: agent_name,
            external_url: Some("https://api.openai.com".to_string()),
            api_key: env::var("INITIAL_AGENT_API_KEY").ok(),
            model: LLMProviderInterface::OpenAI(open_ai),
        };

        let manager = ModelCapabilitiesManager {
            db: db_weak,
            profile: ShinkaiName::new("@@localhost.shinkai/test_profile".to_string()).unwrap(),
            llm_providers: vec![fake_gpt_agent],
        };

        assert!(manager.has_capability(ModelCapability::TextInference).await);
        assert!(!manager.has_capability(ModelCapability::ImageAnalysis).await);
        assert!(!manager.has_capability(ModelCapability::ImageGeneration).await);
    }

    #[test]
    fn test_latest_openai_model_limits() {
        let gpt_5_4 = LLMProviderInterface::OpenAI(OpenAI {
            model_type: "gpt-5.4".to_string(),
        });
        let gpt_5_pro = LLMProviderInterface::OpenAI(OpenAI {
            model_type: "gpt-5-pro".to_string(),
        });
        let o3_pro = LLMProviderInterface::OpenAI(OpenAI {
            model_type: "o3-pro".to_string(),
        });

        assert_eq!(ModelCapabilitiesManager::get_max_tokens(&gpt_5_4), 1_050_000);
        assert_eq!(ModelCapabilitiesManager::get_max_output_tokens(&gpt_5_4), 128_000);
        assert!(ModelCapabilitiesManager::has_reasoning_capabilities(&gpt_5_4));

        assert_eq!(ModelCapabilitiesManager::get_max_tokens(&gpt_5_pro), 400_000);
        assert_eq!(ModelCapabilitiesManager::get_max_output_tokens(&gpt_5_pro), 272_000);
        assert!(ModelCapabilitiesManager::has_reasoning_capabilities(&gpt_5_pro));

        assert_eq!(ModelCapabilitiesManager::get_max_tokens(&o3_pro), 200_000);
        assert_eq!(ModelCapabilitiesManager::get_max_output_tokens(&o3_pro), 100_000);
        assert!(ModelCapabilitiesManager::has_reasoning_capabilities(&o3_pro));
    }

    #[test]
    fn test_latest_claude_and_gemini_limits() {
        let claude = LLMProviderInterface::Claude(Claude {
            model_type: "claude-opus-4-7".to_string(),
        });
        let gemini = LLMProviderInterface::Gemini(Gemini {
            model_type: "gemini-3.1-pro-preview".to_string(),
        });

        assert_eq!(ModelCapabilitiesManager::get_max_tokens(&claude), 1_000_000);
        assert_eq!(ModelCapabilitiesManager::get_max_output_tokens(&claude), 128_000);
        assert!(ModelCapabilitiesManager::has_reasoning_capabilities(&claude));
        assert!(ModelCapabilitiesManager::has_tool_capabilities(&claude, None));

        assert_eq!(ModelCapabilitiesManager::get_max_tokens(&gemini), 1_048_576);
        assert_eq!(ModelCapabilitiesManager::get_max_output_tokens(&gemini), 65_536);
        assert!(ModelCapabilitiesManager::has_reasoning_capabilities(&gemini));
        assert!(ModelCapabilitiesManager::has_tool_capabilities(&gemini, None));
    }

    #[test]
    fn test_latest_grok_and_deepseek_limits() {
        let grok = LLMProviderInterface::Grok(Grok {
            model_type: "grok-4.20-reasoning".to_string(),
        });
        let deepseek = LLMProviderInterface::DeepSeek(DeepSeek {
            model_type: "deepseek-reasoner".to_string(),
        });

        assert_eq!(ModelCapabilitiesManager::get_max_tokens(&grok), 2_000_000);
        assert!(ModelCapabilitiesManager::has_reasoning_capabilities(&grok));
        assert!(ModelCapabilitiesManager::has_tool_capabilities(&grok, None));

        assert_eq!(ModelCapabilitiesManager::get_max_tokens(&deepseek), 128_000);
        assert_eq!(ModelCapabilitiesManager::get_max_output_tokens(&deepseek), 64_000);
        assert!(ModelCapabilitiesManager::has_reasoning_capabilities(&deepseek));
    }
}
