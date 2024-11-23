use crate::{SqliteManager, SqliteManagerError};
use rusqlite::{params, Result};
use serde_json;
use shinkai_tools_primitives::tools::tool_playground::{ToolPlayground, ToolPlaygroundMetadata};

impl SqliteManager {
    // Adds or updates a ToolPlayground entry in the tool_playground table
    pub fn set_tool_playground(&self, tool: &ToolPlayground) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let job_id_history_str = tool.job_id_history.join(",");
        let keywords = tool.metadata.keywords.join(",");
        let configurations = serde_json::to_string(&tool.metadata.configurations)
            .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;
        let parameters = serde_json::to_string(&tool.metadata.parameters)
            .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;
        let result = serde_json::to_string(&tool.metadata.result)
            .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;
        let sql_tables = serde_json::to_string(&tool.metadata.sql_tables)
            .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;
        let sql_queries = serde_json::to_string(&tool.metadata.sql_queries)
            .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;
        // Check if the entry exists
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM tool_playground WHERE tool_router_key = ?1)",
            params![tool.tool_router_key.as_deref()],
            |row| row.get(0),
        )?;

        if exists {
            // Update existing entry
            tx.execute(
                "UPDATE tool_playground SET
                    name = ?1,
                    description = ?2,
                    author = ?3,
                    keywords = ?4,
                    configurations = ?5,
                    parameters = ?6,
                    result = ?7,
                    job_id = ?8,
                    job_id_history = ?9,
                    code = ?10,
                    sql_tables = ?11,
                    sql_queries = ?12
                WHERE tool_router_key = ?13",
                params![
                    tool.metadata.name,
                    tool.metadata.description,
                    tool.metadata.author,
                    keywords,
                    configurations,
                    parameters,
                    result,
                    tool.job_id,
                    job_id_history_str,
                    tool.code,
                    sql_tables,
                    sql_queries,
                    tool.tool_router_key.as_deref(),
                ],
            )?;
        } else {
            // Insert new entry
            tx.execute(
                "INSERT INTO tool_playground (
                    name, description, author, keywords, configurations, parameters, result, tool_router_key, job_id, job_id_history, code, sql_tables, sql_queries
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    tool.metadata.name,
                    tool.metadata.description,
                    tool.metadata.author,
                    keywords,
                    configurations,
                    parameters,
                    result,
                    tool.tool_router_key.as_deref(),
                    tool.job_id,
                    job_id_history_str,
                    tool.code,
                    sql_tables,
                    sql_queries,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    // Removes a ToolPlayground entry and its associated messages from the tool_playground table
    pub fn remove_tool_playground(&self, tool_router_key: &str) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Remove all messages associated with the tool_router_key
        tx.execute(
            "DELETE FROM tool_playground_code_history WHERE tool_router_key = ?1",
            params![tool_router_key],
        )?;

        // Remove the tool playground entry
        tx.execute(
            "DELETE FROM tool_playground WHERE tool_router_key = ?1",
            params![tool_router_key],
        )?;

        tx.commit()?;
        Ok(())
    }

    // Retrieves a ToolPlayground entry based on its tool_router_key
    pub fn get_tool_playground(&self, tool_router_key: &str) -> Result<ToolPlayground, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT name, description, author, keywords, configurations, parameters, result, tool_router_key, job_id, job_id_history, code, sql_tables, sql_queries
             FROM tool_playground WHERE tool_router_key = ?1",
        )?;

        let tool = stmt
            .query_row(params![tool_router_key], |row| {
                let keywords: String = row.get(3)?;
                let configurations: String = row.get(4)?;
                let parameters: String = row.get(5)?;
                let result: String = row.get(6)?;
                let job_id_history: String = row.get(9)?;
                let sql_tables: String = row.get(11)?;
                let sql_queries: String = row.get(12)?;

                let configurations = serde_json::from_str(&configurations).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                let parameters = serde_json::from_str(&parameters).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                let result = serde_json::from_str(&result).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                let sql_tables = serde_json::from_str(&sql_tables).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                let sql_queries = serde_json::from_str(&sql_queries).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;

                Ok(ToolPlayground {
                    metadata: ToolPlaygroundMetadata {
                        name: row.get(0)?,
                        description: row.get(1)?,
                        author: row.get(2)?,
                        keywords: keywords.split(',').map(String::from).collect(),
                        configurations,
                        parameters,
                        result,
                        sql_tables,
                        sql_queries,
                    },
                    tool_router_key: row.get(7)?,
                    job_id: row.get(8)?,
                    job_id_history: job_id_history.split(',').map(String::from).collect(),
                    code: row.get(10)?,
                })
            })
            .map_err(|e| {
                if e == rusqlite::Error::QueryReturnedNoRows {
                    SqliteManagerError::ToolPlaygroundNotFound(tool_router_key.to_string())
                } else {
                    SqliteManagerError::DatabaseError(e)
                }
            })?;

        Ok(tool)
    }

    // Retrieves all ToolPlayground entries from the tool_playground table
    pub fn get_all_tool_playground(&self) -> Result<Vec<ToolPlayground>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT name, description, author, keywords, configurations, parameters, result, tool_router_key, job_id, job_id_history, code, sql_tables, sql_queries
             FROM tool_playground",
        )?;

        let tool_iter = stmt.query_map([], |row| {
            let keywords: String = row.get(3)?;
            let configurations: String = row.get(4)?;
            let parameters: String = row.get(5)?;
            let result: String = row.get(6)?;
            let job_id_history: String = row.get(9)?;
            let sql_tables: String = row.get(11)?;
            let sql_queries: String = row.get(12)?;

            let configurations = serde_json::from_str(&configurations).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let parameters = serde_json::from_str(&parameters).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let result = serde_json::from_str(&result).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let sql_tables = serde_json::from_str(&sql_tables).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let sql_queries = serde_json::from_str(&sql_queries).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            Ok(ToolPlayground {
                metadata: ToolPlaygroundMetadata {
                    name: row.get(0)?,
                    description: row.get(1)?,
                    author: row.get(2)?,
                    keywords: keywords.split(',').map(String::from).collect(),
                    configurations,
                    parameters,
                    result,
                    sql_tables,
                    sql_queries,
                },
                tool_router_key: row.get(7)?,
                job_id: row.get(8)?,
                job_id_history: job_id_history.split(',').map(String::from).collect(),
                code: row.get(10)?,
            })
        })?;

        let mut tools = Vec::new();
        for tool in tool_iter {
            tools.push(tool.map_err(SqliteManagerError::DatabaseError)?);
        }

        Ok(tools)
    }

    // Adds a new entry to the tool_playground_code_history table
    pub fn add_tool_playground_code_history(
        &self,
        message_id: &str,
        tool_router_key: &str,
        code: &str,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO tool_playground_code_history (message_id, tool_router_key, code) VALUES (?1, ?2, ?3)",
            params![message_id, tool_router_key, code],
        )
        .map_err(|e| {
            eprintln!("Database error: {}", e);
            SqliteManagerError::DatabaseError(e)
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_tools_primitives::tools::{
        argument::ToolOutputArg,
        deno_tools::{DenoTool, DenoToolResult},
        shinkai_tool::ShinkaiTool,
    };
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    async fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    async fn add_tool_to_db(manager: &mut SqliteManager) -> String {
        let deno_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Test Tool".to_string(),
            author: "Deno Author".to_string(),
            js_code: "console.log('Hello, Deno!');".to_string(),
            config: vec![],
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: vec![],
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: DenoToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
        };

        let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);
        let vector = SqliteManager::generate_vector_for_testing(0.1);

        // Add the tool to the database
        manager.add_tool_with_vector(shinkai_tool.clone(), vector).unwrap();

        // Return the tool_router_key generated from the DenoTool
        shinkai_tool.tool_router_key().to_string()
    }

    fn create_test_tool_playground(tool_router_key: String) -> ToolPlayground {
        ToolPlayground {
            metadata: ToolPlaygroundMetadata {
                name: "Test Tool".to_string(),
                description: "A tool for testing".to_string(),
                author: "Test Author".to_string(),
                keywords: vec!["test".to_string(), "tool".to_string()],
                configurations: vec![],
                parameters: vec![],
                result: DenoToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
                sql_tables: vec![],
                sql_queries: vec![],
            },
            tool_router_key: Some(tool_router_key),
            job_id: "job_123".to_string(),
            job_id_history: vec![],
            code: "console.log('Hello, world!');".to_string(),
        }
    }

    #[tokio::test]
    async fn test_set_and_get_tool_playground() {
        let mut manager = setup_test_db().await;
        let tool_router_key = add_tool_to_db(&mut manager).await;
        let tool = create_test_tool_playground(tool_router_key.clone());

        // Set the tool playground
        manager.set_tool_playground(&tool).unwrap();

        // Retrieve the tool playground
        let retrieved_tool = manager.get_tool_playground(&tool_router_key).unwrap();

        // Verify the retrieved tool matches the original
        assert_eq!(retrieved_tool.metadata.name, tool.metadata.name);
        assert_eq!(retrieved_tool.metadata.description, tool.metadata.description);
        assert_eq!(retrieved_tool.metadata.author, tool.metadata.author);
        assert_eq!(retrieved_tool.metadata.keywords, tool.metadata.keywords);
        assert_eq!(retrieved_tool.tool_router_key, tool.tool_router_key);
        assert_eq!(retrieved_tool.job_id, tool.job_id);
        assert_eq!(retrieved_tool.code, tool.code);
    }

    #[tokio::test]
    async fn test_remove_tool_playground() {
        let mut manager = setup_test_db().await;
        let tool_router_key = add_tool_to_db(&mut manager).await;
        let tool = create_test_tool_playground(tool_router_key.clone());

        // Set the tool playground
        manager.set_tool_playground(&tool).unwrap();

        // Remove the tool playground
        manager.remove_tool_playground(&tool_router_key).unwrap();

        // Verify the tool playground is removed
        let result = manager.get_tool_playground(&tool_router_key);
        assert!(matches!(result, Err(SqliteManagerError::ToolPlaygroundNotFound(_))));
    }

    #[tokio::test]
    async fn test_get_all_tool_playground() {
        let mut manager = setup_test_db().await;

        // Add the first tool to the database and get its tool_router_key
        let tool_router_key1 = add_tool_to_db_with_unique_name(&mut manager, "Deno Test Tool 1").await;

        // Add the second tool to the database and get its tool_router_key
        let tool_router_key2 = add_tool_to_db_with_unique_name(&mut manager, "Deno Test Tool 2").await;

        // Create ToolPlayground entries using the tool_router_keys
        let tool1 = create_test_tool_playground(tool_router_key1.clone());
        let mut tool2 = create_test_tool_playground(tool_router_key2.clone());
        tool2.job_id = "job_456".to_string();
        tool2.metadata.name = "Another Test Tool".to_string();

        // Set the tool playgrounds
        manager.set_tool_playground(&tool1).unwrap();
        manager.set_tool_playground(&tool2).unwrap();

        // Retrieve all tool playgrounds
        let tools = manager.get_all_tool_playground().unwrap();

        // Verify the number of tools and their contents
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(|t| t.job_id == tool1.job_id));
        assert!(tools.iter().any(|t| t.job_id == tool2.job_id));
    }

    // Helper function to add a tool with a unique name
    async fn add_tool_to_db_with_unique_name(manager: &mut SqliteManager, name: &str) -> String {
        let deno_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: name.to_string(),
            author: "Deno Author".to_string(),
            js_code: "console.log('Hello, Deno!');".to_string(),
            config: vec![],
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: vec![],
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: DenoToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
        };

        let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);
        let vector = SqliteManager::generate_vector_for_testing(0.1);

        // Add the tool to the database
        manager.add_tool_with_vector(shinkai_tool.clone(), vector).unwrap();

        // Return the tool_router_key generated from the DenoTool
        shinkai_tool.tool_router_key().to_string()
    }

    #[tokio::test]
    async fn test_add_tool_and_tool_playground() {
        let mut manager = setup_test_db().await;
        let tool_router_key = add_tool_to_db(&mut manager).await;

        // Step 2: Add a ToolPlayground that references the tool
        let tool_playground = create_test_tool_playground(tool_router_key.clone());

        // Set the tool playground
        manager.set_tool_playground(&tool_playground).unwrap();

        // Retrieve the tool playground
        let retrieved_tool_playground = manager.get_tool_playground(&tool_router_key).unwrap();

        // Verify the retrieved tool playground matches the original
        assert_eq!(retrieved_tool_playground.metadata.name, tool_playground.metadata.name);
        assert_eq!(
            retrieved_tool_playground.metadata.description,
            tool_playground.metadata.description
        );
        assert_eq!(
            retrieved_tool_playground.metadata.author,
            tool_playground.metadata.author
        );
        assert_eq!(
            retrieved_tool_playground.metadata.keywords,
            tool_playground.metadata.keywords
        );
        assert_eq!(
            retrieved_tool_playground.tool_router_key,
            tool_playground.tool_router_key
        );
        assert_eq!(retrieved_tool_playground.job_id, tool_playground.job_id);
        assert_eq!(retrieved_tool_playground.code, tool_playground.code);
    }

    #[tokio::test]
    async fn test_add_and_remove_tool_playground_message() {
        let mut manager = setup_test_db().await;

        // Add a tool to ensure the tool_router_key exists
        let deno_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Test Tool".to_string(),
            author: "Deno Author".to_string(),
            js_code: "console.log('Hello, Deno!');".to_string(),
            config: vec![],
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: vec![],
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: DenoToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
        };

        let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);
        let vector = SqliteManager::generate_vector_for_testing(0.1);
        manager.add_tool_with_vector(shinkai_tool.clone(), vector).unwrap();

        // Create and add a ToolPlayground entry
        let tool_playground = create_test_tool_playground(shinkai_tool.tool_router_key().to_string());
        manager.set_tool_playground(&tool_playground).unwrap();

        // Add a message to the tool_playground_code_history table
        let message_id = "msg-001";
        let code = "console.log('Message Code');";
        manager
            .add_tool_playground_code_history(message_id, &shinkai_tool.tool_router_key(), code)
            .unwrap();

        // Verify the message was added
        let conn = manager.get_connection().unwrap();
        let retrieved_code: String = conn
            .query_row(
                "SELECT code FROM tool_playground_code_history WHERE message_id = ?1",
                params![message_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(retrieved_code, code);

        // Remove the ToolPlayground and its messages
        manager.remove_tool_playground(&shinkai_tool.tool_router_key()).unwrap();

        // Verify the ToolPlayground is removed
        let result = manager.get_tool_playground(&shinkai_tool.tool_router_key());
        assert!(matches!(result, Err(SqliteManagerError::ToolPlaygroundNotFound(_))));

        // Verify the message is removed
        let message_result: Result<String, _> = conn.query_row(
            "SELECT code FROM tool_playground_code_history WHERE message_id = ?1",
            params![message_id],
            |row| row.get(0),
        );

        assert!(matches!(message_result, Err(rusqlite::Error::QueryReturnedNoRows)));
    }
}
