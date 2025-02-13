use shinkai_message_primitives::schemas::mcp_server::{MCPServer, MCPServerType};

use crate::{errors::SqliteManagerError, SqliteManager};

impl SqliteManager {
    pub fn get_all_mcp_servers(&self) -> Result<Vec<MCPServer>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT id, created_at, updated_at, name, type, url, command, is_enabled FROM mcp_servers")?;

        let servers = stmt.query_map([], |row| {
            Ok(MCPServer {
                id: row.get(0)?,
                created_at: row.get(1)?,
                updated_at: row.get(2)?,
                name: row.get(3)?,
                r#type: MCPServerType::from_str(&row.get::<_, String>(4)?).unwrap(),
                url: row.get(5)?,
                command: row.get(6)?,
                is_enabled: row.get::<_, bool>(7)?,
            })  
        })?;

        let mut results = Vec::new();
        for server in servers {
            results.push(server?);
        }

        Ok(results)
    }
}
