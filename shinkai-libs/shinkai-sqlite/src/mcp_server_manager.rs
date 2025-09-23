use shinkai_message_primitives::schemas::mcp_server::{MCPServer, MCPServerEnv, MCPServerType};

use crate::{errors::SqliteManagerError, SqliteManager};

impl SqliteManager {
    pub fn get_all_mcp_servers(&self) -> Result<Vec<MCPServer>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare("SELECT id, created_at, updated_at, name, type, url, command, env, is_enabled FROM mcp_servers")?;

        let servers = stmt.query_map([], |row| {
            Ok(MCPServer {
                id: row.get(0)?,
                created_at: row.get(1)?,
                updated_at: row.get(2)?,
                name: row.get(3)?,
                r#type: MCPServerType::from_str(&row.get::<_, String>(4)?).unwrap(),
                url: row.get(5)?,
                command: row.get(6)?,
                env: {
                    let env_str: Option<String> = row.get(7)?;
                    env_str.map(|s| serde_json::from_str(&s).unwrap_or_default())
                },
                is_enabled: row.get::<_, bool>(8)?,
            })
        })?;

        let mut results = Vec::new();
        for server in servers {
            results.push(server?);
        }

        Ok(results)
    }

    pub fn get_mcp_server(&self, id: i64) -> Result<Option<MCPServer>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, created_at, updated_at, name, type, url, command, env, is_enabled FROM mcp_servers WHERE id = ?",
        )?;
        let mut rows = stmt.query([id])?;
        let row = rows.next()?;
        if let Some(row) = row {
            Ok(Some(MCPServer {
                id: row.get(0)?,
                created_at: row.get(1)?,
                updated_at: row.get(2)?,
                name: row.get(3)?,
                r#type: MCPServerType::from_str(&row.get::<_, String>(4)?).unwrap(),
                url: row.get(5)?,
                command: row.get(6)?,
                env: {
                    let env_str: Option<String> = row.get(7)?;
                    env_str.map(|s| serde_json::from_str(&s).unwrap_or_default())
                },
                is_enabled: row.get::<_, bool>(8)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn add_mcp_server(
        &self,
        id: Option<i64>,
        name: String,
        r#type: MCPServerType,
        url: Option<String>,
        command: Option<String>,
        env: Option<MCPServerEnv>,
        is_enabled: bool,
    ) -> Result<MCPServer, SqliteManagerError> {
        let conn = self.get_connection()?;
        let id: i64 = id.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
        });
        let mut stmt = conn.prepare(
            "INSERT INTO mcp_servers (id, name, type, url, command, env, is_enabled) 
             VALUES (?, ?, ?, ?, ?, ?, ?) 
             RETURNING id, created_at, updated_at, name, type, url, command, env, is_enabled",
        )?;

        let mut rows = stmt.query([
            id.to_string(),
            name.clone(),
            r#type.to_string(),
            url.clone().unwrap_or("".to_string()),
            command.clone().unwrap_or("".to_string()),
            env.as_ref()
                .map(|e| serde_json::to_string(&e).unwrap())
                .unwrap_or_else(|| "{}".to_string()),
            if is_enabled { 1.to_string() } else { 0.to_string() },
        ])?;

        match rows.next()? {
            Some(row) => Ok(MCPServer {
                id: row.get(0)?,
                created_at: row.get(1)?,
                updated_at: row.get(2)?,
                name: row.get(3)?,
                r#type: MCPServerType::from_str(&row.get::<_, String>(4)?).unwrap(),
                url: row.get(5)?,
                command: row.get(6)?,
                env: {
                    let env_str: Option<String> = row.get(7)?;
                    env_str.map(|s| serde_json::from_str(&s).unwrap_or_default())
                },
                is_enabled: row.get::<_, bool>(8)?,
            }),
            None => {
                log::error!("Insert query returned no rows");
                Err(SqliteManagerError::DatabaseError(rusqlite::Error::QueryReturnedNoRows))
            }
        }
    }

    pub fn update_mcp_server(
        &self,
        id: i64,
        name: String,
        r#type: MCPServerType,
        url: Option<String>,
        command: Option<String>,
        env: Option<MCPServerEnv>,
        is_enabled: bool,
    ) -> Result<MCPServer, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("UPDATE mcp_servers SET name = ?, type = ?, url = ?, command = ?, env = ?, is_enabled = ? WHERE id = ? RETURNING id, created_at, updated_at, name, type, url, command, env, is_enabled")?;
        let mut rows = stmt.query([
            name,
            r#type.to_string(),
            url.clone().unwrap_or("".to_string()),
            command.clone().unwrap_or("".to_string()),
            env.map(|e| serde_json::to_string(&e).unwrap())
                .unwrap_or_else(|| "{}".to_string()),
            if is_enabled { 1.to_string() } else { 0.to_string() },
            (id as i64).to_string(),
        ])?;
        match rows.next()? {
            Some(row) => Ok(MCPServer {
                id: row.get(0)?,
                created_at: row.get(1)?,
                updated_at: row.get(2)?,
                name: row.get(3)?,
                r#type: MCPServerType::from_str(&row.get::<_, String>(4)?).unwrap(),
                url: row.get(5)?,
                command: row.get(6)?,
                env: {
                    let env_str: Option<String> = row.get(7)?;
                    env_str.map(|s| serde_json::from_str(&s).unwrap_or_default())
                },
                is_enabled: row.get::<_, bool>(8)?,
            }),
            None => Err(SqliteManagerError::DatabaseError(rusqlite::Error::QueryReturnedNoRows)),
        }
    }

    pub fn delete_mcp_server(&self, id: i64) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM mcp_servers WHERE id = ?")?;
        stmt.execute([id])?;
        Ok(())
    }

    pub fn update_mcp_server_enabled_status(&self, id: i64, is_enabled: bool) -> Result<MCPServer, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "UPDATE mcp_servers SET is_enabled = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? RETURNING id, created_at, updated_at, name, type, url, command, env, is_enabled"
        )?;
        let mut rows = stmt.query([if is_enabled { 1.to_string() } else { 0.to_string() }, id.to_string()])?;
        match rows.next()? {
            Some(row) => Ok(MCPServer {
                id: row.get(0)?,
                created_at: row.get(1)?,
                updated_at: row.get(2)?,
                name: row.get(3)?,
                r#type: MCPServerType::from_str(&row.get::<_, String>(4)?).unwrap(),
                url: row.get(5)?,
                command: row.get(6)?,
                env: {
                    let env_str: Option<String> = row.get(7)?;
                    env_str.map(|s| serde_json::from_str(&s).unwrap_or_default())
                },
                is_enabled: row.get::<_, bool>(8)?,
            }),
            None => Err(SqliteManagerError::DatabaseError(rusqlite::Error::QueryReturnedNoRows)),
        }
    }

    pub fn add_mcp_server_with_id(
        &self,
        id: i64,
        name: String,
        r#type: MCPServerType,
        url: Option<String>,
        command: Option<String>,
        env: Option<MCPServerEnv>,
        is_enabled: bool,
    ) -> Result<MCPServer, SqliteManagerError> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "INSERT INTO mcp_servers (id, name, type, url, command, env, is_enabled) 
             VALUES (?, ?, ?, ?, ?, ?, ?) 
             RETURNING id, created_at, updated_at, name, type, url, command, env, is_enabled",
        )?;

        let mut rows = stmt.query([
            id.to_string(),
            name.clone(),
            r#type.to_string(),
            url.clone().unwrap_or("".to_string()),
            command.clone().unwrap_or("".to_string()),
            env.as_ref()
                .map(|e| serde_json::to_string(&e).unwrap())
                .unwrap_or_else(|| "{}".to_string()),
            if is_enabled { 1.to_string() } else { 0.to_string() },
        ])?;

        match rows.next()? {
            Some(row) => Ok(MCPServer {
                id: row.get(0)?,
                created_at: row.get(1)?,
                updated_at: row.get(2)?,
                name: row.get(3)?,
                r#type: MCPServerType::from_str(&row.get::<_, String>(4)?).unwrap(),
                url: row.get(5)?,
                command: row.get(6)?,
                env: {
                    let env_str: Option<String> = row.get(7)?;
                    env_str.map(|s| serde_json::from_str(&s).unwrap_or_default())
                },
                is_enabled: row.get::<_, bool>(8)?,
            }),
            None => {
                log::error!("Insert query returned no rows");
                Err(SqliteManagerError::DatabaseError(rusqlite::Error::QueryReturnedNoRows))
            }
        }
    }
    pub fn check_if_server_exists(
        &self,
        r#type: &MCPServerType,
        command: String,
        url: String,
    ) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT id, name, type, url, command FROM mcp_servers")?;
        let mut rows = stmt.query([])?;
        let mut result: bool = false;
        while let Some(row) = rows.next()? {
            let server: MCPServer = MCPServer {
                id: row.get(0)?,
                name: row.get(1)?,
                r#type: MCPServerType::from_str(&row.get::<_, String>(2)?).unwrap(),
                url: row.get(3)?,
                command: row.get(4)?,
                created_at: None,
                updated_at: None,
                env: None,
                is_enabled: true,
            };
            match r#type {
                MCPServerType::Command => {
                    let server_command = server.command;
                    if let Some(server_command) = server_command {
                        if server_command.trim() == command.trim() {
                            result = true;
                            break;
                        }
                    }
                }
                MCPServerType::Sse => {
                    let server_url = server.url;
                    if let Some(server_url) = server_url {
                        if server_url.trim() == url.trim() {
                            result = true;
                            break;
                        }
                    }
                }
                MCPServerType::Http => {
                    let server_url = server.url;
                    if let Some(server_url) = server_url {
                        if server_url.trim() == url.trim() {
                            result = true;
                            break;
                        }
                    }
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    async fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type = EmbeddingModelType::default();

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    fn create_test_mcp_server_env() -> MCPServerEnv {
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "test_key_123".to_string());
        env.insert("BASE_URL".to_string(), "https://api.example.com".to_string());
        env
    }

    #[tokio::test]
    async fn test_update_mcp_server() {
        let manager = setup_test_db().await;

        // First, add a server to the database
        let initial_env = create_test_mcp_server_env();
        let original_server = manager
            .add_mcp_server(
                None,
                "Test MCP Server".to_string(),
                MCPServerType::Command,
                Some("http://original.example.com".to_string()),
                Some("npx test-original".to_string()),
                Some(initial_env.clone()),
                true,
            )
            .unwrap();

        let server_id = original_server.id.unwrap();

        // Verify the original server was added correctly
        assert_eq!(original_server.name, "Test MCP Server");
        assert_eq!(original_server.r#type, MCPServerType::Command);
        assert_eq!(original_server.url, Some("http://original.example.com".to_string()));
        assert_eq!(original_server.command, Some("npx test-original".to_string()));
        assert_eq!(original_server.env, Some(initial_env));
        assert!(original_server.is_enabled);

        // Now update the server with new values
        let mut updated_env = HashMap::new();
        updated_env.insert("API_KEY".to_string(), "updated_key_456".to_string());
        updated_env.insert("NEW_CONFIG".to_string(), "new_value".to_string());

        let updated_server = manager
            .update_mcp_server(
                server_id,
                "Updated MCP Server".to_string(),
                MCPServerType::Sse,
                Some("https://updated.example.com".to_string()),
                Some("npx updated-command".to_string()),
                Some(updated_env.clone()),
                false,
            )
            .unwrap();

        // Verify all fields were updated correctly
        assert_eq!(updated_server.id, Some(server_id));
        assert_eq!(updated_server.name, "Updated MCP Server");
        assert_eq!(updated_server.r#type, MCPServerType::Sse);
        assert_eq!(updated_server.url, Some("https://updated.example.com".to_string()));
        assert_eq!(updated_server.command, Some("npx updated-command".to_string()));
        assert_eq!(updated_server.env, Some(updated_env));
        assert!(!updated_server.is_enabled);

        // Verify timestamps exist and are valid (they might be the same due to fast execution)
        assert!(updated_server.created_at.is_some());
        assert!(updated_server.updated_at.is_some());

        // Verify the update persisted by retrieving the server again
        let retrieved_server = manager.get_mcp_server(server_id).unwrap().unwrap();
        assert_eq!(retrieved_server.name, "Updated MCP Server");
        assert_eq!(retrieved_server.r#type, MCPServerType::Sse);
        assert_eq!(retrieved_server.url, Some("https://updated.example.com".to_string()));
        assert_eq!(retrieved_server.command, Some("npx updated-command".to_string()));
        assert!(!retrieved_server.is_enabled);

        // Verify environment variables were updated correctly
        if let Some(env) = retrieved_server.env {
            assert_eq!(env.get("API_KEY").unwrap(), "updated_key_456");
            assert_eq!(env.get("NEW_CONFIG").unwrap(), "new_value");
            assert!(!env.contains_key("BASE_URL")); // Old env var should be gone
        } else {
            panic!("Environment variables should not be None");
        }
    }

    #[tokio::test]
    async fn test_update_mcp_server_with_none_values() {
        let manager = setup_test_db().await;

        // Add a server with some initial values
        let initial_env = create_test_mcp_server_env();
        let original_server = manager
            .add_mcp_server(
                None,
                "Test Server".to_string(),
                MCPServerType::Command,
                Some("http://example.com".to_string()),
                Some("npx test".to_string()),
                Some(initial_env),
                true,
            )
            .unwrap();

        let server_id = original_server.id.unwrap();

        // Update with None values for url, command, and env
        let updated_server = manager
            .update_mcp_server(
                server_id,
                "Updated Server".to_string(),
                MCPServerType::Sse,
                None, // url becomes None
                None, // command becomes None
                None, // env becomes None
                false,
            )
            .unwrap();

        // Verify None values are handled correctly
        assert_eq!(updated_server.name, "Updated Server");
        assert_eq!(updated_server.r#type, MCPServerType::Sse);
        assert_eq!(updated_server.url, Some("".to_string())); // Should be empty string due to unwrap_or
        assert_eq!(updated_server.command, Some("".to_string())); // Should be empty string due to unwrap_or
                                                                  // When env is None, it gets serialized as "{}" and deserialized back as empty HashMap
        assert_eq!(updated_server.env, Some(HashMap::new()));
        assert!(!updated_server.is_enabled);
    }

    #[tokio::test]
    async fn test_update_nonexistent_mcp_server() {
        let manager = setup_test_db().await;

        // Try to update a server that doesn't exist
        let result = manager.update_mcp_server(
            999, // Non-existent ID
            "Non-existent Server".to_string(),
            MCPServerType::Command,
            Some("http://example.com".to_string()),
            Some("npx test".to_string()),
            None,
            true,
        );

        // Should return an error since the server doesn't exist
        assert!(matches!(
            result,
            Err(SqliteManagerError::DatabaseError(rusqlite::Error::QueryReturnedNoRows))
        ));
    }

    #[tokio::test]
    async fn test_update_mcp_server_env_serialization() {
        let manager = setup_test_db().await;

        // Add a server
        let original_server = manager
            .add_mcp_server(
                None,
                "Test Server".to_string(),
                MCPServerType::Command,
                None,
                None,
                None,
                true,
            )
            .unwrap();

        let server_id = original_server.id.unwrap();

        // Create a complex environment with various data types as strings
        let mut complex_env = HashMap::new();
        complex_env.insert("STRING_VAR".to_string(), "simple_string".to_string());
        complex_env.insert("NUMBER_VAR".to_string(), "12345".to_string());
        complex_env.insert("BOOL_VAR".to_string(), "true".to_string());
        complex_env.insert("PATH_VAR".to_string(), "/path/to/something".to_string());
        complex_env.insert("SPECIAL_CHARS".to_string(), "value with spaces & symbols!".to_string());

        // Update with complex environment
        let updated_server = manager
            .update_mcp_server(
                server_id,
                "Complex Env Server".to_string(),
                MCPServerType::Sse,
                None,
                None,
                Some(complex_env.clone()),
                true,
            )
            .unwrap();

        // Verify environment was serialized and deserialized correctly
        assert_eq!(updated_server.env, Some(complex_env));

        // Retrieve and verify persistence
        let retrieved_server = manager.get_mcp_server(server_id).unwrap().unwrap();
        if let Some(env) = retrieved_server.env {
            assert_eq!(env.get("STRING_VAR").unwrap(), "simple_string");
            assert_eq!(env.get("NUMBER_VAR").unwrap(), "12345");
            assert_eq!(env.get("BOOL_VAR").unwrap(), "true");
            assert_eq!(env.get("PATH_VAR").unwrap(), "/path/to/something");
            assert_eq!(env.get("SPECIAL_CHARS").unwrap(), "value with spaces & symbols!");
        } else {
            panic!("Environment should not be None");
        }
    }

    #[tokio::test]
    async fn test_add_update_and_disable_mcp_server_lifecycle() {
        let manager = setup_test_db().await;

        // Step 1: Add an MCP server
        let initial_env = create_test_mcp_server_env();
        let added_server = manager
            .add_mcp_server(
                None,
                "Lifecycle Test Server".to_string(),
                MCPServerType::Command,
                Some("http://initial.example.com".to_string()),
                Some("npx initial-command".to_string()),
                Some(initial_env.clone()),
                true, // Initially enabled
            )
            .unwrap();

        let server_id = added_server.id.unwrap();

        // Verify the server was added correctly
        assert_eq!(added_server.name, "Lifecycle Test Server");
        assert_eq!(added_server.r#type, MCPServerType::Command);
        assert_eq!(added_server.url, Some("http://initial.example.com".to_string()));
        assert_eq!(added_server.command, Some("npx initial-command".to_string()));
        assert_eq!(added_server.env, Some(initial_env));
        assert!(added_server.is_enabled);

        // Step 2: Update the server with new values
        let mut updated_env = HashMap::new();
        updated_env.insert("API_KEY".to_string(), "updated_lifecycle_key".to_string());
        updated_env.insert("ENVIRONMENT".to_string(), "production".to_string());

        let updated_server = manager
            .update_mcp_server(
                server_id,
                "Updated Lifecycle Server".to_string(),
                MCPServerType::Sse,
                Some("https://updated.lifecycle.com".to_string()),
                Some("npx updated-lifecycle-command".to_string()),
                Some(updated_env.clone()),
                true, // Keep enabled during update
            )
            .unwrap();

        // Verify the server was updated correctly
        assert_eq!(updated_server.id, Some(server_id));
        assert_eq!(updated_server.name, "Updated Lifecycle Server");
        assert_eq!(updated_server.r#type, MCPServerType::Sse);
        assert_eq!(updated_server.url, Some("https://updated.lifecycle.com".to_string()));
        assert_eq!(
            updated_server.command,
            Some("npx updated-lifecycle-command".to_string())
        );
        assert_eq!(updated_server.env, Some(updated_env));
        assert!(updated_server.is_enabled);

        // Step 3: Disable the server using the enabled status update method
        let disabled_server = manager.update_mcp_server_enabled_status(server_id, false).unwrap();

        // Verify the server was disabled correctly
        assert_eq!(disabled_server.id, Some(server_id));
        assert_eq!(disabled_server.name, "Updated Lifecycle Server"); // Name should remain the same
        assert_eq!(disabled_server.r#type, MCPServerType::Sse); // Type should remain the same
        assert_eq!(disabled_server.url, Some("https://updated.lifecycle.com".to_string())); // URL should remain the same
        assert_eq!(
            disabled_server.command,
            Some("npx updated-lifecycle-command".to_string())
        ); // Command should remain the same
        assert!(!disabled_server.is_enabled); // Should now be disabled

        // Verify environment was preserved during disable operation
        if let Some(env) = disabled_server.env {
            assert_eq!(env.get("API_KEY").unwrap(), "updated_lifecycle_key");
            assert_eq!(env.get("ENVIRONMENT").unwrap(), "production");
        } else {
            panic!("Environment should be preserved during disable operation");
        }

        // Final verification: Retrieve the server from database to ensure persistence
        let final_server = manager.get_mcp_server(server_id).unwrap().unwrap();
        assert_eq!(final_server.name, "Updated Lifecycle Server");
        assert_eq!(final_server.r#type, MCPServerType::Sse);
        assert!(!final_server.is_enabled);

        // Bonus: Test re-enabling the server
        let re_enabled_server = manager.update_mcp_server_enabled_status(server_id, true).unwrap();
        assert!(re_enabled_server.is_enabled);
        assert_eq!(re_enabled_server.name, "Updated Lifecycle Server"); // All other properties should remain unchanged
    }
}
