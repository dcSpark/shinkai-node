use rusqlite::params;
use shinkai_message_primitives::schemas::{llm_providers::agent::Agent, shinkai_name::ShinkaiName};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn add_agent(&self, agent: Agent, profile: &ShinkaiName) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shinkai_agents WHERE agent_id = ?)",
            &[&agent.agent_id],
            |row| row.get(0),
        )?;

        if exists {
            return Err(SqliteManagerError::DataAlreadyExists);
        }

        // Validate the new ShinkaiName
        let agent_name_str = format!(
            "{}/{}/agent/{}",
            profile.node_name,
            profile.profile_name.clone().unwrap_or_default(),
            agent.agent_id
        );
        let _agent_name = ShinkaiName::new(agent_name_str.clone())
            .map_err(|_| SqliteManagerError::InvalidIdentityName(format!("Invalid ShinkaiName: {}", agent_name_str)))?;

        let knowledge = serde_json::to_string(&agent.knowledge).unwrap();
        let config = agent.config.map(|c| serde_json::to_string(&c).unwrap());
        let tools = serde_json::to_string(&agent.tools).unwrap();

        tx.execute(
            "INSERT INTO shinkai_agents (name, agent_id, full_identity_name, llm_provider_id, ui_description, knowledge, storage_path, tools, debug_mode, config)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                agent.name,
                agent.agent_id,
                agent.full_identity_name.full_name,
                agent.llm_provider_id,
                agent.ui_description,
                knowledge,
                agent.storage_path,
                tools,
                agent.debug_mode,
                config,
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn remove_agent(&self, agent_id: &str) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shinkai_agents WHERE agent_id = ?)",
            &[&agent_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        tx.execute("DELETE FROM shinkai_agents WHERE agent_id = ?", &[&agent_id])?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_all_agents(&self) -> Result<Vec<Agent>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM shinkai_agents")?;
        let agents = stmt.query_map([], |row| {
            let full_identity_name: String = row.get(2)?;
            let knowledge: String = row.get(5)?;
            let tools: String = row.get(7)?;
            let config: Option<String> = row.get(9)?;

            Ok(Agent {
                agent_id: row.get(0)?,
                name: row.get(1)?,
                full_identity_name: ShinkaiName::new(full_identity_name).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                llm_provider_id: row.get(3)?,
                ui_description: row.get(4)?,
                knowledge: serde_json::from_str(&knowledge).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                storage_path: row.get(6)?,
                tools: serde_json::from_str(&tools).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                debug_mode: row.get(8)?,
                config: match config {
                    Some(c) => Some(serde_json::from_str(&c).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?),
                    None => None,
                },
            })
        })?;

        let mut result = Vec::new();
        for agent in agents {
            result.push(agent?);
        }

        Ok(result)
    }

    pub fn get_agent(&self, agent_id: &str) -> Result<Option<Agent>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM shinkai_agents WHERE agent_id = ?")?;
        let agent = stmt.query_row(&[&agent_id], |row| {
            let full_identity_name: String = row.get(2)?;
            let knowledge: String = row.get(5)?;
            let tools: String = row.get(7)?;
            let config: Option<String> = row.get(9)?;

            Ok(Agent {
                agent_id: row.get(0)?,
                name: row.get(1)?,
                full_identity_name: ShinkaiName::new(full_identity_name).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                llm_provider_id: row.get(3)?,
                ui_description: row.get(4)?,
                knowledge: serde_json::from_str(&knowledge).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                storage_path: row.get(6)?,
                tools: serde_json::from_str(&tools).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                debug_mode: row.get(8)?,
                config: match config {
                    Some(c) => Some(serde_json::from_str(&c).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?),
                    None => None,
                },
            })
        });

        match agent {
            Ok(agent) => Ok(Some(agent)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SqliteManagerError::DatabaseError(e)),
        }
    }

    pub fn update_agent(&self, updated_agent: Agent) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shinkai_agents WHERE agent_id = ?)",
            &[&updated_agent.agent_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        let knowledge = serde_json::to_string(&updated_agent.knowledge).unwrap();
        let config = updated_agent.config.map(|c| serde_json::to_string(&c).unwrap());
        let tools = serde_json::to_string(&updated_agent.tools).unwrap();

        tx.execute(
            "UPDATE shinkai_agents
            SET name = ?1, full_identity_name = ?2, llm_provider_id = ?3, ui_description = ?4, knowledge = ?5, storage_path = ?6, tools = ?7, debug_mode = ?8, config = ?9
            WHERE agent_id = ?10",
            params![
                updated_agent.name,
                updated_agent.full_identity_name.full_name,
                updated_agent.llm_provider_id,
                updated_agent.ui_description,
                knowledge,
                updated_agent.storage_path,
                tools,
                updated_agent.debug_mode,
                config,
                updated_agent.agent_id,
            ],
        )?;

        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[test]
    fn test_add_agent() {
        let db = setup_test_db();
        let agent = Agent {
            agent_id: "test_agent".to_string(),
            name: "Test Agent".to_string(),
            full_identity_name: ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap(),
            llm_provider_id: "test_llm_provider".to_string(),
            ui_description: "Test description".to_string(),
            knowledge: Default::default(),
            storage_path: "test_storage_path".to_string(),
            tools: Default::default(),
            debug_mode: false,
            config: None,
        };
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        let result = db.add_agent(agent.clone(), &profile);
        assert!(result.is_ok());

        let result = db.add_agent(agent.clone(), &profile);
        assert!(matches!(result, Err(SqliteManagerError::DataAlreadyExists)));
    }

    #[test]
    fn test_remove_agent() {
        let db = setup_test_db();
        let agent = Agent {
            agent_id: "test_agent".to_string(),
            name: "Test Agent".to_string(),
            full_identity_name: ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap(),
            llm_provider_id: "test_llm_provider".to_string(),
            ui_description: "Test description".to_string(),
            knowledge: Default::default(),
            storage_path: "test_storage_path".to_string(),
            tools: Default::default(),
            debug_mode: false,
            config: None,
        };
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        db.add_agent(agent.clone(), &profile).unwrap();

        let result = db.remove_agent(&agent.agent_id);
        assert!(result.is_ok());

        let result = db.remove_agent(&agent.agent_id);
        assert!(matches!(result, Err(SqliteManagerError::DataNotFound)));
    }

    #[test]
    fn test_get_all_agents() {
        let db = setup_test_db();
        let agent1 = Agent {
            agent_id: "test_agent1".to_string(),
            name: "Test Agent 1".to_string(),
            full_identity_name: ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap(),
            llm_provider_id: "test_llm_provider1".to_string(),
            ui_description: "Test description 1".to_string(),
            knowledge: Default::default(),
            storage_path: "test_storage_path1".to_string(),
            tools: Default::default(),
            debug_mode: false,
            config: None,
        };
        let agent2 = Agent {
            agent_id: "test_agent2".to_string(),
            name: "Test Agent 2".to_string(),
            full_identity_name: ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap(),
            llm_provider_id: "test_llm_provider2".to_string(),
            ui_description: "Test description 2".to_string(),
            knowledge: Default::default(),
            storage_path: "test_storage_path2".to_string(),
            tools: Default::default(),
            debug_mode: false,
            config: None,
        };
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        db.add_agent(agent1.clone(), &profile).unwrap();
        db.add_agent(agent2.clone(), &profile).unwrap();

        let agents = db.get_all_agents().unwrap();
        assert_eq!(agents.len(), 2);
        assert!(agents.iter().any(|a| a.agent_id == agent1.agent_id));
        assert!(agents.iter().any(|a| a.agent_id == agent2.agent_id));
    }

    #[test]
    fn test_get_agent() {
        let db = setup_test_db();
        let agent = Agent {
            agent_id: "test_agent".to_string(),
            name: "Test Agent".to_string(),
            full_identity_name: ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap(),
            llm_provider_id: "test_llm_provider".to_string(),
            ui_description: "Test description".to_string(),
            knowledge: Default::default(),
            storage_path: "test_storage_path".to_string(),
            tools: Default::default(),
            debug_mode: false,
            config: None,
        };
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        db.add_agent(agent.clone(), &profile).unwrap();

        let result = db.get_agent(&agent.agent_id).unwrap();
        assert_eq!(result.unwrap().agent_id, agent.agent_id);

        let result = db.get_agent("non_existent_agent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_agent() {
        let db = setup_test_db();
        let agent = Agent {
            agent_id: "test_agent".to_string(),
            name: "Test Agent".to_string(),
            full_identity_name: ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap(),
            llm_provider_id: "test_llm_provider".to_string(),
            ui_description: "Test description".to_string(),
            knowledge: Default::default(),
            storage_path: "test_storage_path".to_string(),
            tools: Default::default(),
            debug_mode: false,
            config: None,
        };
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        db.add_agent(agent.clone(), &profile).unwrap();

        let updated_agent = Agent {
            agent_id: "test_agent".to_string(),
            name: "Updated Test Agent".to_string(),
            full_identity_name: ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap(),
            llm_provider_id: "updated_test_llm_provider".to_string(),
            ui_description: "Updated test description".to_string(),
            knowledge: Default::default(),
            storage_path: "updated_test_storage_path".to_string(),
            tools: Default::default(),
            debug_mode: true,
            config: None,
        };

        let result = db.update_agent(updated_agent.clone());
        assert!(result.is_ok());

        let result = db.get_agent(&updated_agent.agent_id).unwrap();
        let agent = result.unwrap();
        assert_eq!(agent.name, updated_agent.name);
        assert_eq!(agent.llm_provider_id, updated_agent.llm_provider_id);
        assert_eq!(agent.ui_description, updated_agent.ui_description);
        assert_eq!(agent.storage_path, updated_agent.storage_path);
        assert_eq!(agent.debug_mode, updated_agent.debug_mode);
    }
}