use shinkai_message_primitives::schemas::mcp_server::{MCPServer, MCPServerType, MCPServerEnv};

use crate::{errors::SqliteManagerError, SqliteManager};

impl SqliteManager {
    pub fn get_all_mcp_servers(&self) -> Result<Vec<MCPServer>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT id, created_at, updated_at, name, type, url, command, env, is_enabled FROM mcp_servers")?;

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
        name: String,
        r#type: MCPServerType,
        url: Option<String>,
        command: Option<String>,
        env: Option<MCPServerEnv>,
        is_enabled: bool,
    ) -> Result<MCPServer, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO mcp_servers (name, type, url, command, env, is_enabled) 
             VALUES (?, ?, ?, ?, ?, ?) 
             RETURNING id, created_at, updated_at, name, type, url, command, env, is_enabled",
        )?;

        let mut rows = stmt.query([
            name.clone(),
            r#type.to_string(),
            url.clone().unwrap_or("".to_string()),
            command.clone().unwrap_or("".to_string()),
            serde_json::to_string(&env).unwrap_or("{}".to_string()),
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

    pub fn delete_mcp_server(&self, id: i64) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM mcp_servers WHERE id = ?")?;
        stmt.execute([id])?;
        Ok(())
    }
}
