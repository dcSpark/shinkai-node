use rusqlite::params;
use shinkai_message_primitives::schemas::{
    llm_providers::serialized_llm_provider::SerializedLLMProvider, shinkai_name::ShinkaiName
};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    /// Returns the the first half of the blake3 hash of the llm provider id value
    pub fn llm_provider_id_to_hash(llm_provider_id: &str) -> String {
        let full_hash = blake3::hash(llm_provider_id.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    pub fn get_all_llm_providers(&self) -> Result<Vec<SerializedLLMProvider>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM llm_providers")?;
        let llm_providers = stmt.query_map([], |row| {
            let full_identity_name: String = row.get(2)?;
            let model: String = row.get(5)?;
            Ok(SerializedLLMProvider {
                id: row.get(1)?,
                name: row.get(6)?,
                description: row.get(7)?,
                full_identity_name: ShinkaiName::new(full_identity_name).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                external_url: row.get(3)?,
                api_key: row.get(4)?,
                model: serde_json::from_str(&model).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
            })
        })?;

        let mut result = Vec::new();
        for llm_provider in llm_providers {
            result.push(llm_provider?);
        }

        Ok(result)
    }

    pub fn add_llm_provider(
        &self,
        llm_provider: SerializedLLMProvider,
        profile: &ShinkaiName,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("INSERT INTO llm_providers (db_llm_provider_id, id, full_identity_name, external_url, api_key, model, name, description) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")?;

        let llm_provider_id = Self::db_llm_provider_id(&llm_provider.id, profile)?;
        let model = serde_json::to_string(&llm_provider.model)
            .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;
        stmt.execute(params![
            &llm_provider_id,
            &llm_provider.id,
            &llm_provider.full_identity_name.full_name,
            &llm_provider.external_url,
            &llm_provider.api_key,
            &model,
            &llm_provider.name,
            &llm_provider.description,
        ])?;

        Ok(())
    }

    pub fn update_llm_provider(
        &self,
        updated_llm_provider: SerializedLLMProvider,
        profile: &ShinkaiName,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("UPDATE llm_providers SET full_identity_name = ?2, external_url = ?3, api_key = ?4, model = ?5, name = ?6, description = ?7 WHERE db_llm_provider_id = ?1")?;

        let llm_provider_id = Self::db_llm_provider_id(&updated_llm_provider.id, profile)?;
        let model = serde_json::to_string(&updated_llm_provider.model)
            .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;
        stmt.execute(params![
            &llm_provider_id,
            &updated_llm_provider.full_identity_name.full_name,
            &updated_llm_provider.external_url,
            &updated_llm_provider.api_key,
            &model,
            &updated_llm_provider.name,
            &updated_llm_provider.description,
        ])?;

        Ok(())
    }

    pub fn remove_llm_provider(&self, llm_provider_id: &str, profile: &ShinkaiName) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let llm_provider_id = Self::db_llm_provider_id(llm_provider_id, profile)?;

        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM llm_providers WHERE db_llm_provider_id = ?1)",
            params![&llm_provider_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        conn.execute(
            "DELETE FROM llm_providers WHERE db_llm_provider_id = ?1",
            params![&llm_provider_id],
        )?;

        Ok(())
    }

    pub fn get_llm_provider(
        &self,
        llm_provider_id: &str,
        profile: &ShinkaiName,
    ) -> Result<Option<SerializedLLMProvider>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let llm_provider_id = Self::db_llm_provider_id(llm_provider_id, profile)?;
        let mut stmt = conn.prepare("SELECT * FROM llm_providers WHERE db_llm_provider_id = ?1")?;
        let llm_providers = stmt.query_map(params![&llm_provider_id], |row| {
            let full_identity_name: String = row.get(2)?;
            let model: String = row.get(5)?;
            Ok(SerializedLLMProvider {
                id: row.get(1)?,
                name: row.get(6)?,
                description: row.get(7)?,
                full_identity_name: ShinkaiName::new(full_identity_name).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                external_url: row.get(3)?,
                api_key: row.get(4)?,
                model: serde_json::from_str(&model).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
            })
        })?;

        let mut result = Vec::new();
        for llm_provider in llm_providers {
            result.push(llm_provider?);
        }

        if result.is_empty() {
            return Err(SqliteManagerError::DataNotFound);
        }

        Ok(result.pop())
    }

    pub fn get_llm_providers_for_profile(
        &self,
        profile_name: ShinkaiName,
    ) -> Result<Vec<SerializedLLMProvider>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let profile_name = profile_name
            .get_profile_name_string()
            .ok_or(SqliteManagerError::InvalidIdentityName(
                profile_name.full_name.to_string(),
            ))?;
        let mut stmt = conn.prepare("SELECT * FROM llm_providers WHERE db_llm_provider_id LIKE '%:::' || ?1")?;
        let llm_providers = stmt.query_map(params![profile_name], |row| {
            let full_identity_name: String = row.get(2)?;
            let model: String = row.get(5)?;
            Ok(SerializedLLMProvider {
                id: row.get(1)?,
                name: row.get(6)?,
                description: row.get(7)?,
                full_identity_name: ShinkaiName::new(full_identity_name).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                external_url: row.get(3)?,
                api_key: row.get(4)?,
                model: serde_json::from_str(&model).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
            })
        })?;

        let mut result = Vec::new();
        for llm_provider in llm_providers {
            result.push(llm_provider?);
        }

        Ok(result)
    }

    pub fn get_llm_provider_profiles_with_access(
        &self,
        llm_provider_id: &str,
        profile: &ShinkaiName,
    ) -> Result<Vec<String>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let llm_provider_id = Self::db_llm_provider_id(llm_provider_id, profile)?;
        let mut stmt = conn.prepare("SELECT full_identity_name FROM llm_providers WHERE db_llm_provider_id = ?1")?;

        let rows = stmt.query_map(params![&llm_provider_id], |row| {
            let full_identity_name = row.get::<_, String>(0)?;
            ShinkaiName::new(full_identity_name).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })
        })?;

        let mut identities = Vec::new();
        for row in rows {
            identities.push(row?);
        }

        let profiles = identities
            .into_iter()
            .map(|identity| identity.get_profile_name_string().unwrap_or_default())
            .collect();

        Ok(profiles)
    }

    fn db_llm_provider_id(llm_provider_id: &str, profile: &ShinkaiName) -> Result<String, SqliteManagerError> {
        let profile_name = profile
            .get_profile_name_string()
            .clone()
            .ok_or(SqliteManagerError::InvalidIdentityName(profile.full_name.to_string()))?;

        Ok(format!("{}:::{}", llm_provider_id, profile_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use shinkai_message_primitives::schemas::{
        llm_providers::serialized_llm_provider::{LLMProviderInterface, OpenAI}, shinkai_name::ShinkaiName
    };
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[test]
    fn test_add_llm_provider() {
        let db = setup_test_db();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };
        let identity = ShinkaiName::new("@@alice.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap();
        let profile = identity.extract_profile().unwrap();

        let test_agent = SerializedLLMProvider {
            id: "test_agent".to_string(),
            name: Some("Test Agent".to_string()),
            description: Some("A test agent for unit tests.".to_string()),
            full_identity_name: identity,
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
        };

        db.add_llm_provider(test_agent.clone(), &profile)
            .expect("Failed to add new agent");
        let retrieved_agent = db
            .get_llm_provider(&test_agent.id, &profile)
            .expect("Failed to get llm provider");
        assert_eq!(test_agent, retrieved_agent.expect("Failed to retrieve agent"));
    }

    #[test]
    fn test_update_llm_provider() {
        let db = setup_test_db();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };
        let identity = ShinkaiName::new("@@alice.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap();
        let profile = identity.extract_profile().unwrap();

        let test_agent = SerializedLLMProvider {
            id: "test_agent".to_string(),
            name: Some("Test Agent".to_string()),
            description: Some("A test agent for unit tests.".to_string()),
            full_identity_name: identity.clone(),
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai.clone()),
        };

        db.add_llm_provider(test_agent.clone(), &profile)
            .expect("Failed to add new agent");

        let updated_agent = SerializedLLMProvider {
            id: "test_agent".to_string(),
            name: Some("Test Agent Updated".to_string()),
            description: Some("An updated test agent for unit tests.".to_string()),
            full_identity_name: identity,
            external_url: Some("http://localhost:8090".to_string()),
            api_key: Some("test_api_key2".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
        };

        db.update_llm_provider(updated_agent.clone(), &profile)
            .expect("Failed to update agent");
        let retrieved_agent = db
            .get_llm_provider(&test_agent.id, &profile)
            .expect("Failed to get llm provider");
        assert_eq!(updated_agent, retrieved_agent.expect("Failed to retrieve agent"));
    }

    #[test]
    fn test_remove_llm_provider() {
        let db = setup_test_db();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };
        let identity = ShinkaiName::new("@@alice.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap();
        let profile = identity.extract_profile().unwrap();

        let test_agent = SerializedLLMProvider {
            id: "test_agent".to_string(),
            name: Some("Test Agent".to_string()),
            description: Some("A test agent for unit tests.".to_string()),
            full_identity_name: identity.clone(),
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai.clone()),
        };

        db.add_llm_provider(test_agent.clone(), &profile)
            .expect("Failed to add new agent");

        db.remove_llm_provider(&test_agent.id, &profile)
            .expect("Failed to remove agent");

        let retrieved_agent_result = db.get_llm_provider(&test_agent.id, &profile);
        match retrieved_agent_result {
            Ok(_) => panic!("Expected error, but got Ok"),
            Err(e) => assert!(
                matches!(e, SqliteManagerError::DataNotFound),
                "Expected FailedFetchingValue error"
            ),
        }
    }

    #[test]
    fn test_get_all_llm_providers() {
        let db = setup_test_db();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };
        let identity = ShinkaiName::new("@@alice.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap();
        let profile = identity.extract_profile().unwrap();

        let test_agent1 = SerializedLLMProvider {
            id: "test_agent".to_string(),
            name: Some("Test Agent 1".to_string()),
            description: Some("First test agent.".to_string()),
            full_identity_name: identity.clone(),
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai.clone()),
        };

        let test_agent2 = SerializedLLMProvider {
            id: "test_agent2".to_string(),
            name: Some("Test Agent 2".to_string()),
            description: Some("Second test agent.".to_string()),
            full_identity_name: identity.clone(),
            external_url: Some("http://localhost:8081".to_string()),
            api_key: Some("test_api_key2".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai.clone()),
        };

        db.add_llm_provider(test_agent1.clone(), &profile)
            .expect("Failed to add new agent");

        db.add_llm_provider(test_agent2.clone(), &profile)
            .expect("Failed to add new agent");

        let retrieved_agents = db.get_all_llm_providers().expect("Failed to get all llm providers");
        assert_eq!(vec![test_agent1, test_agent2], retrieved_agents);
    }

    #[test]
    fn test_get_llm_providers_for_profile() {
        let db = setup_test_db();
        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo".to_string(),
        };
        let identity1 = ShinkaiName::new("@@alice.shinkai/profileName1/agent/myChatGPTAgent".to_string()).unwrap();
        let identity2 = ShinkaiName::new("@@bob.shinkai/profileName2/agent/myChatGPTAgent2".to_string()).unwrap();
        let profile1 = identity1.extract_profile().unwrap();
        let profile2 = identity2.extract_profile().unwrap();

        let test_agent1 = SerializedLLMProvider {
            id: "test_agent".to_string(),
            name: Some("Test Agent 1".to_string()),
            description: Some("First test agent.".to_string()),
            full_identity_name: identity1.clone(),
            external_url: Some("http://localhost:8080".to_string()),
            api_key: Some("test_api_key".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai.clone()),
        };

        let test_agent2 = SerializedLLMProvider {
            id: "test_agent2".to_string(),
            name: Some("Test Agent 2".to_string()),
            description: Some("Second test agent.".to_string()),
            full_identity_name: identity2.clone(),
            external_url: Some("http://localhost:8081".to_string()),
            api_key: Some("test_api_key2".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai.clone()),
        };

        db.add_llm_provider(test_agent1.clone(), &profile1)
            .expect("Failed to add new agent");

        db.add_llm_provider(test_agent2.clone(), &profile2)
            .expect("Failed to add new agent");

        let retrieved_agents = db
            .get_llm_providers_for_profile(profile1)
            .expect("Failed to get llm providers for profile");
        assert_eq!(vec![test_agent1], retrieved_agents);

        let retrieved_agents = db
            .get_llm_providers_for_profile(profile2)
            .expect("Failed to get llm providers for profile");
        assert_eq!(vec![test_agent2], retrieved_agents);
    }
}
