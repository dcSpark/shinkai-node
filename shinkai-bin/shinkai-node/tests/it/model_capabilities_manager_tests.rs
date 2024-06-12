#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, OpenAI, SerializedAgent};
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
    use shinkai_node::db::ShinkaiDB;
    use shinkai_node::managers::model_capabilities_manager::{
        ModelCapability, ModelCost, ModelPrivacy, ModelCapabilitiesManager,
    };
    use std::path::Path;
    use std::sync::Arc;
    use std::{env, fs};

    #[ignore]
    fn setup() {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(&path);
    }

    #[tokio::test]
    async fn test_has_capability() {
        init_default_tracing(); 
        setup();
        let db = Arc::new(ShinkaiDB::new("db_tests/").unwrap());
        let db_weak = Arc::downgrade(&db);

        let agent_id = "agent_id1".to_string();
        let agent_name =
            ShinkaiName::new(format!("@@localhost.shinkai/main/agent/{}", agent_id.clone()).to_string()).unwrap();

        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo-1106".to_string(),
        };

        let gpt_3_5_agent = SerializedAgent {
            id: agent_id.clone(),
            full_identity_name: agent_name,
            perform_locally: false,
            external_url: Some("https://api.openai.com".to_string()),
            api_key: env::var("INITIAL_AGENT_API_KEY").ok(),
            model: AgentLLMInterface::OpenAI(open_ai),
            toolkit_permissions: vec![],
            storage_bucket_permissions: vec![],
            allowed_message_senders: vec![],
        };

        let manager = ModelCapabilitiesManager {
            db: db_weak,
            profile: ShinkaiName::new("@@localhost.shinkai/test_profile".to_string()).unwrap(),
            agents: vec![gpt_3_5_agent.clone()],
        };

        assert!(manager.has_capability(ModelCapability::TextInference).await);
        assert!(!manager.has_capability(ModelCapability::ImageAnalysis).await);

        let capabilities = ModelCapabilitiesManager::get_capability(&gpt_3_5_agent);
        assert_eq!(capabilities.0, vec![ModelCapability::TextInference]);
        assert_eq!(capabilities.1, ModelCost::Cheap);
        assert_eq!(capabilities.2, ModelPrivacy::RemoteGreedy);
    }

    #[tokio::test]
    async fn test_gpt_4_vision_preview_capabilities() {
        init_default_tracing(); 
        setup();
        let db = Arc::new(ShinkaiDB::new("db_tests/").unwrap());
        let db_weak = Arc::downgrade(&db);

        let agent_id = "agent_id2".to_string();
        let agent_name =
            ShinkaiName::new(format!("@@localhost.shinkai/main/agent/{}", agent_id.clone()).to_string()).unwrap();

        let open_ai = OpenAI {
            model_type: "gpt-4-vision-preview".to_string(),
        };

        let gpt_4_vision_agent = SerializedAgent {
            id: agent_id.clone(),
            full_identity_name: agent_name,
            perform_locally: false,
            external_url: Some("https://api.openai.com".to_string()),
            api_key: env::var("INITIAL_AGENT_API_KEY").ok(),
            model: AgentLLMInterface::OpenAI(open_ai),
            toolkit_permissions: vec![],
            storage_bucket_permissions: vec![],
            allowed_message_senders: vec![],
        };

        let manager = ModelCapabilitiesManager {
            db: db_weak,
            profile: ShinkaiName::new("@@localhost.shinkai/test_profile".to_string()).unwrap(),
            agents: vec![gpt_4_vision_agent],
        };

        assert!(manager.has_capability(ModelCapability::TextInference).await);
        assert!(manager.has_capability(ModelCapability::ImageAnalysis).await);
        assert!(!manager.has_capability(ModelCapability::ImageGeneration).await);
    }

    #[tokio::test]
    async fn test_fake_gpt_model_capabilities() {
        init_default_tracing(); 
        setup();
        let db = Arc::new(ShinkaiDB::new("db_tests/").unwrap());
        let db_weak = Arc::downgrade(&db);

        let agent_id = "agent_id3".to_string();
        let agent_name =
            ShinkaiName::new(format!("@@localhost.shinkai/main/agent/{}", agent_id.clone()).to_string()).unwrap();

        let open_ai = OpenAI {
            model_type: "gpt-fake-model".to_string(),
        };

        let fake_gpt_agent = SerializedAgent {
            id: agent_id.clone(),
            full_identity_name: agent_name,
            perform_locally: false,
            external_url: Some("https://api.openai.com".to_string()),
            api_key: env::var("INITIAL_AGENT_API_KEY").ok(),
            model: AgentLLMInterface::OpenAI(open_ai),
            toolkit_permissions: vec![],
            storage_bucket_permissions: vec![],
            allowed_message_senders: vec![],
        };

        let manager = ModelCapabilitiesManager {
            db: db_weak,
            profile: ShinkaiName::new("@@localhost.shinkai/test_profile".to_string()).unwrap(),
            agents: vec![fake_gpt_agent],
        };

        assert!(manager.has_capability(ModelCapability::TextInference).await);
        assert!(!manager.has_capability(ModelCapability::ImageAnalysis).await);
        assert!(!manager.has_capability(ModelCapability::ImageGeneration).await);
    }
}