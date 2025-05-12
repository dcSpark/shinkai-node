use rusqlite::params;
use shinkai_message_primitives::schemas::{llm_providers::agent::Agent, shinkai_name::ShinkaiName};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn add_agent(&self, agent: Agent, profile: &ShinkaiName) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shinkai_agents WHERE agent_id = ?)",
            [&agent.agent_id],
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
        let tools: Vec<String> = agent.tools.iter().map(|t| t.to_string_with_version()).collect();
        let tools = serde_json::to_string(&tools).unwrap();
        let scope = serde_json::to_string(&agent.scope).unwrap();
        let tools_config_override = agent.tools_config_override.map(|c| serde_json::to_string(&c).unwrap());

        tx.execute(
            "INSERT INTO shinkai_agents (name, agent_id, full_identity_name, llm_provider_id, ui_description, knowledge, storage_path, tools, debug_mode, config, scope, tools_config_override, edited)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                agent.name,
                agent.agent_id.to_lowercase(),
                agent.full_identity_name.full_name,
                agent.llm_provider_id,
                agent.ui_description,
                knowledge,
                agent.storage_path,
                tools,
                agent.debug_mode,
                config,
                scope,
                tools_config_override,
                agent.edited,
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
            [&agent_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        tx.execute("DELETE FROM shinkai_agents WHERE agent_id = ?", [&agent_id])?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_all_agents(&self) -> Result<Vec<Agent>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT agent_id, name, full_identity_name, llm_provider_id, ui_description, knowledge, storage_path, tools, debug_mode, config, scope, tools_config_override, edited FROM shinkai_agents")?;
        let agents = stmt.query_map([], |row| {
            let agent_id: String = row.get(0)?;
            let full_identity_name: String = row.get(2)?;
            let knowledge: String = row.get(5)?;
            let tools: String = row.get(7)?;
            let config: Option<String> = row.get(9)?;
            let scope: String = row.get(10)?;
            let tools_config_override: Option<String> = row.get(11).unwrap_or(None);
            let edited: bool = row.get(12)?;
            let avatar_url: Option<String> = row.get(12)?;
            Ok(Agent {
                agent_id: agent_id.clone(),
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
                scope: serde_json::from_str(&scope).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                cron_tasks: None,
                tools_config_override: match tools_config_override {
                    Some(c) => Some(serde_json::from_str(&c).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?),
                    None => None,
                },
                avatar_url: avatar_url,
                edited: edited,
            })
        })?;

        let mut result = Vec::new();
        for agent in agents {
            result.push(agent?);
        }

        Ok(result)
    }

    pub fn get_agent(&self, agent_id: &str) -> Result<Option<Agent>, SqliteManagerError> {
        let conn: r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager> = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT agent_id, name, full_identity_name, llm_provider_id, ui_description, knowledge, storage_path, tools, debug_mode, config, scope, tools_config_override, edited, avatar_url FROM shinkai_agents WHERE agent_id = ?")?;
        let agent = stmt.query_row([&agent_id], |row| {
            let agent_id: String = row.get(0)?;
            let full_identity_name: String = row.get(2)?;
            let knowledge: String = row.get(5)?;
            let tools: String = row.get(7)?;
            let config: Option<String> = row.get(9)?;
            let scope: String = row.get(10)?;
            let tools_config_override: Option<String> = row.get(11).unwrap_or(None);
            let avatar_url: Option<String> = row.get(13)?;
            let debug_mode: bool = row.get(8)?;
            let edited: bool = row.get(12)?;
            let storage_path: String = row.get(6)?;
            Ok(Agent {
                agent_id: agent_id.clone(),
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
                storage_path,
                tools: serde_json::from_str(&tools).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                debug_mode,
                config: match config {
                    Some(c) => Some(serde_json::from_str(&c).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?),
                    None => None,
                },
                scope: serde_json::from_str(&scope).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                cron_tasks: None,
                tools_config_override: match tools_config_override {
                    Some(c) => Some(serde_json::from_str(&c).map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                            e.to_string(),
                        )))
                    })?),
                    None => None,
                },
                avatar_url: avatar_url,
                edited: edited,
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
            [&updated_agent.agent_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        let knowledge = serde_json::to_string(&updated_agent.knowledge).unwrap();
        let config = updated_agent.config.map(|c| serde_json::to_string(&c).unwrap());
        let tools: Vec<String> = updated_agent.tools.iter().map(|t| t.to_string_with_version()).collect();
        let tools = serde_json::to_string(&tools).unwrap();
        let scope = serde_json::to_string(&updated_agent.scope).unwrap();
        let tools_config_override = updated_agent
            .tools_config_override
            .map(|c| serde_json::to_string(&c).unwrap());

        tx.execute(
            "UPDATE shinkai_agents
            SET name = ?1, full_identity_name = ?2, llm_provider_id = ?3, ui_description = ?4, knowledge = ?5, storage_path = ?6, tools = ?7, debug_mode = ?8, config = ?9, scope = ?10, tools_config_override = ?11, edited = ?12
            WHERE agent_id = ?13",
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
                scope,
                tools_config_override,
                1,
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
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use shinkai_message_primitives::schemas::{shinkai_name::ShinkaiName, tool_router_key::ToolRouterKey};
    use std::collections::HashMap;
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
            scope: Default::default(),
            cron_tasks: None,
            tools_config_override: None,
            avatar_url: None,
            edited: false,
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
            scope: Default::default(),
            cron_tasks: None,
            tools_config_override: None,
            avatar_url: None,
            edited: false,
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
            scope: Default::default(),
            cron_tasks: None,
            tools_config_override: None,
            avatar_url: None,
            edited: false,
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
            scope: Default::default(),
            cron_tasks: None,
            tools_config_override: None,
            avatar_url: None,
            edited: false,
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
        let expected_agent = Agent {
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
            scope: Default::default(),
            cron_tasks: None,
            tools_config_override: None,
            avatar_url: None,
            edited: false,
        };
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        db.add_agent(expected_agent.clone(), &profile).unwrap();

        let result = db.get_agent(&expected_agent.agent_id).unwrap();
        assert!(result.is_some());
        let agent = result.unwrap();

        assert_eq!(agent.agent_id, expected_agent.agent_id);
        assert_eq!(agent.name, expected_agent.name);
        assert_eq!(agent.full_identity_name, expected_agent.full_identity_name);
    }

    #[test]
    fn test_get_agent_not_found() {
        let db = setup_test_db();

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
            scope: Default::default(),
            cron_tasks: None,
            tools_config_override: None,
            avatar_url: None,
            edited: false,
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
            scope: Default::default(),
            cron_tasks: None,
            tools_config_override: None,
            avatar_url: None,
            edited: true,
        };

        let result = db.update_agent(updated_agent.clone());
        assert!(result.is_ok());

        let result = db.get_agent(&updated_agent.agent_id).unwrap();
        let agent = result.unwrap();
        assert_eq!(agent.name, updated_agent.name);
        assert_eq!(agent.llm_provider_id, updated_agent.llm_provider_id);
        assert_eq!(agent.ui_description, updated_agent.ui_description);
        assert_eq!(agent.storage_path, updated_agent.storage_path);
    }

    #[test]
    fn test_agent_with_tool_config_override() {
        let db = setup_test_db();

        // Create a proper ToolRouterKey
        let tool = ToolRouterKey::new(
            "local".to_string(),
            "__author_shinkai".to_string(),
            "test_tool".to_string(),
            Some("1.0".to_string()),
        );

        // Create a tool configuration override map
        let mut tool_config = HashMap::new();
        let mut params = HashMap::new();
        params.insert("api_key".to_string(), serde_json::Value::String("test_key".to_string()));
        params.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(30)),
        );
        tool_config.insert(tool.to_string_with_version(), params);

        let agent = Agent {
            agent_id: "test_agent_with_config".to_string(),
            name: "Test Agent With Config".to_string(),
            full_identity_name: ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap(),
            llm_provider_id: "test_llm_provider".to_string(),
            ui_description: "Test description".to_string(),
            knowledge: Default::default(),
            storage_path: "test_storage_path".to_string(),
            tools: vec![tool.clone()],
            debug_mode: false,
            config: None,
            scope: Default::default(),
            cron_tasks: None,
            tools_config_override: Some(tool_config),
            avatar_url: None,
            edited: false,
        };
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        // Add the agent
        db.add_agent(agent.clone(), &profile).unwrap();

        // Retrieve the agent
        let retrieved_agent = db.get_agent(&agent.agent_id).unwrap().unwrap();

        // Verify the tools_config_override was correctly stored and retrieved
        assert!(retrieved_agent.tools_config_override.is_some());
        let retrieved_config = retrieved_agent.tools_config_override.unwrap();
        assert!(retrieved_config.contains_key(&tool.to_string_with_version()));
        let params = retrieved_config.get(&tool.to_string_with_version()).unwrap();
        assert_eq!(params.get("api_key").unwrap().as_str().unwrap(), "test_key");
        assert_eq!(params.get("timeout").unwrap().as_i64().unwrap(), 30);
        assert!(!retrieved_agent.edited);
    }

    #[test]
    fn test_agent_with_tool_config_override_edit() {
        let db = setup_test_db();

        // Create a proper ToolRouterKey
        let tool = ToolRouterKey::new(
            "local".to_string(),
            "__author_shinkai".to_string(),
            "test_tool".to_string(),
            Some("1.0".to_string()),
        );

        // Create a tool configuration override map
        let mut tool_config = HashMap::new();
        let mut params = HashMap::new();
        params.insert("api_key".to_string(), serde_json::Value::String("test_key".to_string()));
        params.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(30)),
        );
        tool_config.insert(tool.to_string_with_version(), params);

        let agent = Agent {
            agent_id: "test_agent_with_config".to_string(),
            name: "Test Agent With Config".to_string(),
            full_identity_name: ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap(),
            llm_provider_id: "test_llm_provider".to_string(),
            ui_description: "Test description".to_string(),
            knowledge: Default::default(),
            storage_path: "test_storage_path".to_string(),
            tools: vec![tool.clone()],
            debug_mode: false,
            config: None,
            scope: Default::default(),
            cron_tasks: None,
            tools_config_override: Some(tool_config),
            edited: true,
        };
        let profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        // Add the agent
        db.add_agent(agent.clone(), &profile).unwrap();

        // Retrieve the agent
        let retrieved_agent = db.get_agent(&agent.agent_id).unwrap().unwrap();

        // Verify the tools_config_override was correctly stored and retrieved
        assert!(retrieved_agent.tools_config_override.is_some());
        let retrieved_config = retrieved_agent.tools_config_override.unwrap();
        assert!(retrieved_config.contains_key(&tool.to_string_with_version()));
        let params = retrieved_config.get(&tool.to_string_with_version()).unwrap();
        assert_eq!(params.get("api_key").unwrap().as_str().unwrap(), "test_key");
        assert_eq!(params.get("timeout").unwrap().as_i64().unwrap(), 30);
        assert!(retrieved_agent.edited);
    }
}
