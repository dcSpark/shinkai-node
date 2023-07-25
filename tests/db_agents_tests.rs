use shinkai_node::{
    db::{db_errors::ShinkaiDBError, ShinkaiDB},
    managers::{agent::AgentAPIModel, agent_serialization::SerializedAgent, providers::openai::OpenAI},
    shinkai_message::utils::hash_string,
};
use std::fs;
use std::path::Path;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_remove_agent() {
        setup();
        // Initialize ShinkaiDB
        let db_path = format!("db_tests/{}", hash_string("agent_test".clone()));
        let mut db = ShinkaiDB::new(&db_path).unwrap();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };

        // Create an instance of SerializedAgent
        let test_agent = SerializedAgent {
            id: "test_agent".to_string(),
            name: "test_name".to_string(),
            perform_locally: false,
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: AgentAPIModel::OpenAI(open_ai),
            toolkit_permissions: vec!["toolkit1".to_string(), "toolkit2".to_string()],
            storage_bucket_permissions: vec!["storage1".to_string(), "storage2".to_string()],
            allowed_message_senders: vec!["sender1".to_string(), "sender2".to_string()],
        };

        // Add a new agent
        db.add_agent(test_agent.clone()).expect("Failed to add new agent");
        let retrieved_agent = db.get_agent(&test_agent.id).expect("Failed to get agent");
        assert_eq!(test_agent, retrieved_agent.expect("Failed to retrieve agent"));

        // Remove the agent
        let result = db.remove_agent(&test_agent.id);
        assert!(result.is_ok(), "Failed to remove agent");

        // Attempt to get the removed agent, expecting an error
        let retrieved_agent = db.get_agent(&test_agent.id).expect("Failed to get agent");
        assert_eq!(None, retrieved_agent);

        // Attempt to remove the same agent again, expecting an error
        let result = db.remove_agent(&test_agent.id);
        assert!(
            matches!(result, Err(ShinkaiDBError::SomeError)),
            "Expected SomeError error"
        );
    }

    #[test]
    fn test_update_agent_access() {
        setup();
        // Initialize ShinkaiDB
        let db_path = format!("db_tests/{}", hash_string("agent_test".clone()));
        let mut db = ShinkaiDB::new(&db_path).unwrap();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };

        // Create an instance of SerializedAgent
        let test_agent = SerializedAgent {
            id: "test_agent".to_string(),
            name: "test_name".to_string(),
            perform_locally: false,
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: AgentAPIModel::OpenAI(open_ai),
            toolkit_permissions: vec!["toolkit1".to_string(), "toolkit2".to_string()],
            storage_bucket_permissions: vec!["storage1".to_string(), "storage2".to_string()],
            allowed_message_senders: vec!["sender1".to_string(), "sender2".to_string()],
        };

        // Add a new agent
        db.add_agent(test_agent.clone()).expect("Failed to add new agent");

        // Update agent access
        let result = db.update_agent_access(
            &test_agent.id,
            Some(vec!["new_sender".to_string()]),
            Some(vec!["new_toolkit".to_string()]),
        );
        assert!(result.is_ok(), "Failed to update agent access");

        // Attempt to update access for a non-existent agent, expecting an error
        let result = db.update_agent_access(
            "non_existent_agent",
            Some(vec!["new_sender".to_string()]),
            Some(vec!["new_toolkit".to_string()]),
        );
        assert!(
            matches!(result, Err(ShinkaiDBError::SomeError)),
            "Expected SomeError error"
        );
    }

    #[test]
    fn test_get_agent_profiles_and_toolkits() {
        setup();
        let db_path = format!("db_tests/{}", hash_string("agent_test".clone()));
        let mut db = ShinkaiDB::new(&db_path).unwrap();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };

        let test_agent = SerializedAgent {
            id: "test_agent".to_string(),
            name: "test_name".to_string(),
            perform_locally: false,
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: AgentAPIModel::OpenAI(open_ai),
            toolkit_permissions: vec!["toolkit1".to_string(), "toolkit2".to_string()],
            storage_bucket_permissions: vec!["storage1".to_string(), "storage2".to_string()],
            allowed_message_senders: vec!["sender1".to_string(), "sender2".to_string()],
        };

        // Add a new agent
        db.add_agent(test_agent.clone()).expect("Failed to add new agent");

        // Get agent profiles with access
        let profiles = db.get_agent_profiles_with_access(&test_agent.id);
        assert!(profiles.is_ok(), "Failed to get agent profiles");
        assert_eq!(vec!["sender1", "sender2"], profiles.unwrap());

        // Get agent toolkits accessible
        let toolkits = db.get_agent_toolkits_accessible(&test_agent.id);
        assert!(toolkits.is_ok(), "Failed to get agent toolkits");
        assert_eq!(vec!["toolkit1", "toolkit2"], toolkits.unwrap());
    }

    #[test]
    fn test_remove_profile_and_toolkit_from_agent_access() {
        setup();
        let db_path = format!("db_tests/{}", hash_string("agent_test".clone()));
        let mut db = ShinkaiDB::new(&db_path).unwrap();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };

        let test_agent = SerializedAgent {
            id: "test_agent".to_string(),
            name: "test_name".to_string(),
            perform_locally: false,
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: AgentAPIModel::OpenAI(open_ai),
            toolkit_permissions: vec!["toolkit1".to_string(), "toolkit2".to_string()],
            storage_bucket_permissions: vec!["storage1".to_string(), "storage2".to_string()],
            allowed_message_senders: vec!["sender1".to_string(), "sender2".to_string()],
        };

        // Add a new agent
        db.add_agent(test_agent.clone()).expect("Failed to add new agent");

        // Remove a profile from agent access
        let result = db.remove_profile_from_agent_access(&test_agent.id, "sender1");
        assert!(result.is_ok(), "Failed to remove profile from agent access");
        let profiles = db.get_agent_profiles_with_access(&test_agent.id).unwrap();
        assert_eq!(vec!["sender2"], profiles);

        // Remove a toolkit from agent access
        let result = db.remove_toolkit_from_agent_access(&test_agent.id, "toolkit1");
        assert!(result.is_ok(), "Failed to remove toolkit from agent access");
        let toolkits = db.get_agent_toolkits_accessible(&test_agent.id).unwrap();
        assert_eq!(vec!["toolkit2"], toolkits);
    }
}
