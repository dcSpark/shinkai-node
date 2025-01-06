use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
use shinkai_sqlite::SqliteManager;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::NamedTempFile;

fn setup_test_db() -> SqliteManager {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = PathBuf::from(temp_file.path());
    let api_url = String::new();
    let model_type =
        EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

    SqliteManager::new(db_path, api_url, model_type).unwrap()
}

#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::{
        llm_providers::serialized_llm_provider::{LLMProviderInterface, OpenAI, SerializedLLMProvider},
        shinkai_name::ShinkaiName,
    };
    use shinkai_sqlite::errors::SqliteManagerError;

    use super::*;

    #[tokio::test]
    async fn test_add_and_remove_agent() {
        let db = setup_test_db();
        let db = Arc::new(db);
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };
        let identity = ShinkaiName::new("@@alice.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap();
        let profile = identity.extract_profile().unwrap();

        // Create an instance of SerializedLLMProvider
        let test_agent = SerializedLLMProvider {
            id: "test_agent".to_string(),
            full_identity_name: identity,
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
        };

        // Add a new agent
        db.add_llm_provider(test_agent.clone(), &profile)
            .expect("Failed to add new agent");
        let retrieved_agent = db
            .get_llm_provider(&test_agent.id, &profile)
            .expect("Failed to get llm provider");
        assert_eq!(test_agent, retrieved_agent.expect("Failed to retrieve agent"));

        // Call get_all_llm_providers and check that it returns the right agent
        let all_llm_providers = db.get_all_llm_providers().expect("Failed to get all llm providers");
        assert!(
            all_llm_providers.contains(&test_agent),
            "get_all_llm_providers did not return the added agent"
        );

        // Call get_llm_providers_for_profile and check that it returns the right agent for the profile
        let llm_providers_for_profile = db
            .get_llm_providers_for_profile(profile.clone())
            .expect("Failed to get llm providers for profile");
        assert!(
            llm_providers_for_profile.contains(&test_agent),
            "get_llm_providers_for_profile did not return the added agent"
        );

        // Remove the agent
        let result = db.remove_llm_provider(&test_agent.id, &profile);
        assert!(result.is_ok(), "Failed to remove agent");

        // Attempt to get the removed agent, expecting an error
        let retrieved_agent_result = db.get_llm_provider(&test_agent.id, &profile);
        match retrieved_agent_result {
            Ok(_) => panic!("Expected error, but got Ok"),
            Err(e) => assert!(
                matches!(e, SqliteManagerError::DataNotFound),
                "Expected FailedFetchingValue error"
            ),
        }

        // Attempt to remove the same agent again, expecting an error
        let result = db.remove_llm_provider(&test_agent.id, &profile);
        assert!(
            matches!(result, Err(SqliteManagerError::DataNotFound)),
            "Expected SqliteManagerError error"
        );
    }

    #[tokio::test]
    async fn test_get_agent_profiles_and_toolkits() {
        let db = setup_test_db();
        let db = Arc::new(db);
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo-1106".to_string(),
        };
        let identity = ShinkaiName::new("@@alice.shinkai/profileName/agent/test_name".to_string()).unwrap();
        let profile = identity.extract_profile().unwrap();

        let test_agent = SerializedLLMProvider {
            id: "test_agent".to_string(),
            full_identity_name: identity,
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
        };

        // Add a new agent
        db.add_llm_provider(test_agent.clone(), &profile)
            .expect("Failed to add new agent");

        // Get agent profiles with access
        let profiles = db.get_llm_provider_profiles_with_access(&test_agent.id, &profile);
        assert!(profiles.is_ok(), "Failed to get agent profiles");
        assert_eq!(vec!["profilename"], profiles.unwrap());
    }
}
