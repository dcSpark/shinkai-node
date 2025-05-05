#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
        LLMProviderInterface, OpenAI, SerializedLLMProvider,
    };
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

    use shinkai_node::managers::model_capabilities_manager::{
        ModelCapabilitiesManager, ModelCapability, ModelCost, ModelPrivacy,
    };
    use std::env;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    use shinkai_sqlite::SqliteManager;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM);

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
}
