use mockito::Server;
use shinkai_node::db::{db_errors::ShinkaiDBError, ShinkaiDB};
use std::fs;
use std::path::Path;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::{
        llm_providers::serialized_llm_provider::{LLMProviderInterface, OpenAI, SerializedLLMProvider},
        shinkai_name::ShinkaiName,
    };
    use shinkai_node::llm_provider::{execution::prompts::prompts::JobPromptGenerator, llm_provider::LLMProvider};
    use shinkai_vector_resources::utils::hash_string;

    use super::*;

    #[test]
    fn test_add_and_remove_agent() {
        setup();
        // Initialize ShinkaiDB
        let db_path = format!("db_tests/{}", hash_string("agent_test"));
        let db = ShinkaiDB::new(&db_path).unwrap();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };
        let identity = ShinkaiName::new("@@alice.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap();
        let profile = identity.extract_profile().unwrap();

        // Create an instance of SerializedLLMProvider
        let test_agent = SerializedLLMProvider {
            id: "test_agent".to_string(),
            full_identity_name: identity,
            perform_locally: false,
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
            toolkit_permissions: vec!["toolkit1".to_string(), "toolkit2".to_string()],
            storage_bucket_permissions: vec!["storage1".to_string(), "storage2".to_string()],
            allowed_message_senders: vec!["sender1".to_string(), "sender2".to_string()],
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
                matches!(e, ShinkaiDBError::DataNotFound),
                "Expected FailedFetchingValue error"
            ),
        }

        // Attempt to remove the same agent again, expecting an error
        let result = db.remove_llm_provider(&test_agent.id, &profile);
        assert!(
            matches!(result, Err(ShinkaiDBError::DataNotFound)),
            "Expected RocksDBError error"
        );
    }

    #[test]
    fn test_update_agent_access() {
        setup();
        // Initialize ShinkaiDB
        let db_path = format!("db_tests/{}", hash_string("agent_test"));
        let db = ShinkaiDB::new(&db_path).unwrap();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo-1106".to_string(),
        };
        let identity = ShinkaiName::new("@@alice.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap();
        let profile = identity.extract_profile().unwrap();

        // Create an instance of SerializedLLMProvider
        let test_agent = SerializedLLMProvider {
            id: "test_agent".to_string(),
            full_identity_name: identity,
            perform_locally: false,
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
            toolkit_permissions: vec!["toolkit1".to_string(), "toolkit2".to_string()],
            storage_bucket_permissions: vec!["storage1".to_string(), "storage2".to_string()],
            allowed_message_senders: vec!["sender1".to_string(), "sender2".to_string()],
        };

        // Add a new agent
        db.add_llm_provider(test_agent.clone(), &profile)
            .expect("Failed to add new agent");

        // Update agent access
        let result = db.update_llm_provider_access(
            &test_agent.id,
            &profile,
            Some(vec!["new_sender".to_string()]),
            Some(vec!["new_toolkit".to_string()]),
        );
        assert!(result.is_ok(), "Failed to update agent access");

        // Attempt to update access for a non-existent agent, expecting an error
        let result = db.update_llm_provider_access(
            "non_existent_agent",
            &profile,
            Some(vec!["new_sender".to_string()]),
            Some(vec!["new_toolkit".to_string()]),
        );
        eprintln!("Result: {:?}", result);
        assert!(
            matches!(result, Err(ShinkaiDBError::DataNotFound)),
            "Expected ColumnFamilyNotFound error"
        );
    }

    #[test]
    fn test_get_agent_profiles_and_toolkits() {
        setup();
        let db_path = format!("db_tests/{}", hash_string("agent_test"));
        let db = ShinkaiDB::new(&db_path).unwrap();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo-1106".to_string(),
        };
        let identity = ShinkaiName::new("@@alice.shinkai/profileName/agent/test_name".to_string()).unwrap();
        let profile = identity.extract_profile().unwrap();

        let test_agent = SerializedLLMProvider {
            id: "test_agent".to_string(),
            full_identity_name: identity,
            perform_locally: false,
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
            toolkit_permissions: vec!["toolkit1".to_string(), "toolkit2".to_string()],
            storage_bucket_permissions: vec!["storage1".to_string(), "storage2".to_string()],
            allowed_message_senders: vec!["sender1".to_string(), "sender2".to_string()],
        };

        // Add a new agent
        db.add_llm_provider(test_agent.clone(), &profile)
            .expect("Failed to add new agent");

        // Get agent profiles with access
        let profiles = db.get_llm_provider_profiles_with_access(&test_agent.id, &profile);
        assert!(profiles.is_ok(), "Failed to get agent profiles");
        assert_eq!(vec!["profilename", "sender1", "sender2"], profiles.unwrap());

        // Get agent toolkits accessible
        let toolkits = db.get_llm_provider_toolkits_accessible(&test_agent.id, &profile);
        assert!(toolkits.is_ok(), "Failed to get agent toolkits");
        assert_eq!(vec!["toolkit1", "toolkit2"], toolkits.unwrap());
    }

    #[test]
    fn test_remove_profile_and_toolkit_from_agent_access() {
        setup();
        let db_path = format!("db_tests/{}", hash_string("agent_test"));
        let db = ShinkaiDB::new(&db_path).unwrap();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo-1106".to_string(),
        };
        let identity = ShinkaiName::new("@@alice.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap();
        let profile = identity.extract_profile().unwrap();
        eprintln!("Profile: {:?}", profile);

        let test_agent = SerializedLLMProvider {
            id: "test_agent".to_string(),
            full_identity_name: identity,
            perform_locally: false,
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
            toolkit_permissions: vec!["toolkit1".to_string(), "toolkit2".to_string()],
            storage_bucket_permissions: vec!["storage1".to_string(), "storage2".to_string()],
            allowed_message_senders: vec!["sender1".to_string(), "sender2".to_string()],
        };

        // Add a new agent
        db.add_llm_provider(test_agent.clone(), &profile)
            .expect("Failed to add new agent");

        // Remove a profile from agent access
        let result = db.remove_profile_from_llm_provider_access(&test_agent.id, "sender1", &profile);
        assert!(result.is_ok(), "Failed to remove profile from agent access");
        let profiles = db
            .get_llm_provider_profiles_with_access(&test_agent.id, &profile)
            .unwrap();
        assert_eq!(vec!["profilename", "sender2"], profiles);

        // Remove a toolkit from agent access
        let result = db.remove_toolkit_from_llm_provider_access(&test_agent.id, "toolkit1", &profile);
        assert!(result.is_ok(), "Failed to remove toolkit from agent access");
        let toolkits = db
            .get_llm_provider_toolkits_accessible(&test_agent.id, &profile)
            .unwrap();
        assert_eq!(vec!["toolkit2"], toolkits);
    }

    #[tokio::test]
    async fn test_agent_call_external_api_openai() {
        let mut server = Server::new();
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
                            "content": "Hello there, how may I assist you today?"
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

        let openai = OpenAI {
            model_type: "gpt-3.5-turbo-1106".to_string(),
        };
        let agent = LLMProvider::new(
            "1".to_string(),
            ShinkaiName::new("@@alice.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap(),
            false,
            Some(server.url()), // use the url of the mock server
            Some("mockapikey".to_string()),
            LLMProviderInterface::OpenAI(openai),
            vec!["tk1".to_string(), "tk2".to_string()],
            vec!["sb1".to_string(), "sb2".to_string()],
            vec!["allowed1".to_string(), "allowed2".to_string()],
        );

        let response = agent
            .inference(
                JobPromptGenerator::basic_instant_response_prompt("Hello!".to_string(), None),
                None,
                None,
                None,
            )
            .await;
        match response {
            Ok(res) => assert_eq!(
                res.response_string.as_str().to_string(),
                "Hello there, how may I assist you today?".to_string()
            ),
            Err(e) => panic!("Error when calling API: {}", e),
        }
    }
}
