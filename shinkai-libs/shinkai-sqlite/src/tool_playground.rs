use crate::{SqliteManager, SqliteManagerError};
use rusqlite::{params, Result};
use serde_json;
use shinkai_message_primitives::schemas::{indexable_version::IndexableVersion, shinkai_tools::CodeLanguage};
use shinkai_tools_primitives::tools::tool_playground::{ToolPlayground, ToolPlaygroundMetadata};

impl SqliteManager {
    // Adds or updates a ToolPlayground entry in the tool_playground table
    pub fn set_tool_playground(&self, tool: &ToolPlayground) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // 1) Make sure tool_key exists at all
        let row_exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shinkai_tools WHERE tool_key = ?1)",
            params![tool.tool_router_key.as_deref()],
            |row| row.get(0),
        )?;
        if !row_exists {
            return Err(SqliteManagerError::ToolKeyNotFound(
                tool.tool_router_key.clone().unwrap_or_default(),
            ));
        }

        // 2) Find the highest version for that tool_key
        let tool_version: i64 = tx.query_row(
            "SELECT version FROM shinkai_tools 
             WHERE tool_key = ?1 
             ORDER BY version DESC 
             LIMIT 1",
            params![tool.tool_router_key.as_deref()],
            |row| row.get(0),
        )?;

        // Convert tool.metadata.version to IndexableVersion
        let tool_version_indexable = IndexableVersion::from_number(tool_version as u64);
        let tool_metadata_version_indexable = IndexableVersion::from_string(&tool.metadata.version)?;

        // Check if the tool's metadata version matches the tool_version
        if tool_metadata_version_indexable.to_version_string() != tool_version_indexable.to_version_string() {
            return Err(SqliteManagerError::VersionMismatch {
                expected: tool_version_indexable.to_version_string(),
                found: tool.metadata.version.clone(),
            });
        }

        // Prepare JSON fields
        let job_id_history_str = tool.job_id_history.join(",");
        let keywords = tool.metadata.keywords.join(",");
        let configurations = serde_json::to_string(&tool.metadata.configurations)
            .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;
        let parameters = serde_json::to_string(&tool.metadata.parameters)
            .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;
        let result = serde_json::to_string(&tool.metadata.result)
            .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;

        // Check if an entry already exists for this router_key
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM tool_playground 
                           WHERE tool_router_key = ?1)",
            params![tool.tool_router_key.as_deref()],
            |row| row.get(0),
        )?;

        if exists {
            // Update existing
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
                    language = ?11,
                    tool_version = ?12
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
                    tool.language.to_string(),
                    tool_version, // <= we ensure the current highest version
                    tool.tool_router_key.as_deref()
                ],
            )?;
        } else {
            // Insert new
            tx.execute(
                "INSERT INTO tool_playground (
                    name, description, author, keywords, configurations,
                    parameters, result,
                    tool_router_key, tool_version,
                    job_id, job_id_history, code, language
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                          ?8, ?9,
                          ?10, ?11, ?12, ?13)",
                params![
                    tool.metadata.name,
                    tool.metadata.description,
                    tool.metadata.author,
                    keywords,
                    configurations,
                    parameters,
                    result,
                    tool.tool_router_key.as_deref(),
                    tool_version, // new code: we must insert the version
                    tool.job_id,
                    job_id_history_str,
                    tool.code,
                    tool.language.to_string(),
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Removes a ToolPlayground entry and its associated messages
    pub fn remove_tool_playground(&self, tool_router_key: &str) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // 1) Look up the tool_playground.id for this router_key
        let playground_id: i64 = match tx.query_row(
            "SELECT id FROM tool_playground WHERE tool_router_key = ?1",
            params![tool_router_key],
            |row| row.get(0),
        ) {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(SqliteManagerError::ToolPlaygroundNotFound(tool_router_key.to_string()))
            }
            Err(e) => return Err(SqliteManagerError::DatabaseError(e)),
        };

        // 2) Remove all messages referencing this id
        tx.execute(
            "DELETE FROM tool_playground_code_history WHERE tool_playground_id = ?1",
            params![playground_id],
        )?;

        // 3) Remove the playground entry
        tx.execute("DELETE FROM tool_playground WHERE id = ?1", params![playground_id])?;

        tx.commit()?;
        Ok(())
    }

    /// Retrieves a ToolPlayground by router_key (still no version needed for the call)
    pub fn get_tool_playground(&self, tool_router_key: &str) -> Result<ToolPlayground, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT 
                name, description, author, keywords, configurations, parameters,
                result, tool_router_key, job_id, job_id_history, code, language, tool_version
             FROM tool_playground 
             WHERE tool_router_key = ?1
             ORDER BY tool_version DESC
             LIMIT 1",
        )?;

        let tool = stmt
            .query_row(params![tool_router_key], |row| {
                let keywords: String = row.get(3)?;
                let configurations: String = row.get(4)?;
                let parameters: String = row.get(5)?;
                let result: String = row.get(6)?;
                let job_id_history: String = row.get(9)?;
                let language: String = row.get(11)?;
                let version: IndexableVersion = IndexableVersion::from_number(row.get(12)?);

                let code_language = match language.as_str() {
                    "typescript" => CodeLanguage::Typescript,
                    "python" => CodeLanguage::Python,
                    _ => CodeLanguage::Typescript,
                };

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

                let mut sql_tables = vec![];
                let mut sql_queries = vec![];
                let mut tools = None;
                let mut oauth = None;
                let mut assets = None;
                let mut homepage: Option<String> = None;
                if let Ok(tool_data) = self.get_tool_by_key(tool_router_key) {
                    // found data
                    sql_queries = tool_data.sql_queries();
                    sql_tables = tool_data.sql_tables();
                    tools = Some(tool_data.get_tools());
                    oauth = tool_data.get_oauth();
                    assets = tool_data.get_assets();
                    homepage = tool_data.get_homepage();
                }

                Ok(ToolPlayground {
                    language: code_language,
                    metadata: ToolPlaygroundMetadata {
                        name: row.get(0)?,
                        homepage,
                        version: version.to_version_string(),
                        description: row.get(1)?,
                        author: row.get(2)?,
                        keywords: keywords.split(',').map(String::from).collect(),
                        configurations,
                        parameters,
                        result,
                        sql_tables,
                        sql_queries,
                        tools,
                        oauth,
                    },
                    tool_router_key: row.get(7)?,
                    job_id: row.get(8)?,
                    job_id_history: job_id_history.split(',').map(String::from).collect(),
                    code: row.get(10)?,
                    assets,
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

    /// A helper for listing all tool_playgrounds (version usage is behind the scenes)
    pub fn get_all_tool_playground(&self) -> Result<Vec<ToolPlayground>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                name, description, author, keywords, configurations, parameters, 
                result, tool_router_key, job_id, job_id_history, code, language, tool_version
             FROM tool_playground",
        )?;

        let tool_iter = stmt.query_map([], |row| {
            let keywords: String = row.get(3)?;
            let configurations: String = row.get(4)?;
            let parameters: String = row.get(5)?;
            let result: String = row.get(6)?;
            let job_id_history: String = row.get(9)?;
            let language: String = row.get(11)?;
            let version: IndexableVersion = IndexableVersion::from_number(row.get(12)?);
            let code_language = match language.as_str() {
                "typescript" => CodeLanguage::Typescript,
                "python" => CodeLanguage::Python,
                _ => CodeLanguage::Typescript,
            };

            // same deserialization pattern
            let configurations = serde_json::from_str(&configurations).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let parameters = serde_json::from_str(&parameters).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let result = serde_json::from_str(&result).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            Ok(ToolPlayground {
                language: code_language,
                metadata: ToolPlaygroundMetadata {
                    name: row.get(0)?,
                    homepage: None,
                    version: version.to_version_string(),
                    description: row.get(1)?,
                    author: row.get(2)?,
                    keywords: keywords.split(',').map(String::from).collect(),
                    configurations,
                    parameters,
                    result,
                    sql_tables: vec![],
                    sql_queries: vec![],
                    tools: None,
                    oauth: None,
                },
                tool_router_key: row.get(7)?,
                job_id: row.get(8)?,
                job_id_history: job_id_history.split(',').map(String::from).collect(),
                code: row.get(10)?,
                assets: None,
            })
        })?;

        let mut tools = Vec::new();
        for tool_row in tool_iter {
            tools.push(tool_row.map_err(SqliteManagerError::DatabaseError)?);
        }
        Ok(tools)
    }

    /// Add a new entry to code_history by looking up the `id` from tool_playground
    pub fn add_tool_playground_code_history(
        &self,
        message_id: &str,
        tool_router_key: &str,
        code: &str,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;

        // 1) Find the ID of the playground row
        let playground_id: i64 = conn
            .query_row(
                "SELECT id FROM tool_playground WHERE tool_router_key = ?1 order by tool_version desc limit 1",
                params![tool_router_key],
                |row| row.get(0),
            )
            .map_err(|e| {
                if e == rusqlite::Error::QueryReturnedNoRows {
                    SqliteManagerError::ToolPlaygroundNotFound(tool_router_key.to_string())
                } else {
                    SqliteManagerError::DatabaseError(e)
                }
            })?;

        // 2) Insert into code_history
        conn.execute(
            "INSERT INTO tool_playground_code_history (message_id, tool_playground_id, code)
             VALUES (?1, ?2, ?3)",
            params![message_id, playground_id, code],
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
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use shinkai_tools_primitives::tools::{
        deno_tools::{DenoTool, ToolResult},
        parameters::Parameters,
        shinkai_tool::ShinkaiTool,
        tool_output_arg::ToolOutputArg,
    };
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
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            name: "Deno Test Tool".to_string(),
            author: "Deno Author".to_string(),
            version: "1.0.0".to_string(),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![],
            config: vec![],
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            oauth: None,
            assets: None,
        };

        let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);
        let vector = SqliteManager::generate_vector_for_testing(0.1);

        // Add the tool to the database
        manager.add_tool_with_vector(shinkai_tool.clone(), vector).unwrap();

        // Return the tool_router_key generated from the DenoTool
        shinkai_tool.tool_router_key().to_string_without_version()
    }

    fn create_test_tool_playground(tool_router_key: String) -> ToolPlayground {
        ToolPlayground {
            language: CodeLanguage::Typescript,
            metadata: ToolPlaygroundMetadata {
                name: "Test Tool".to_string(),
                homepage: Some("http://127.0.0.1/index.html".to_string()),
                version: "1.0.0".to_string(),
                description: "A tool for testing".to_string(),
                author: "Test Author".to_string(),
                keywords: vec!["test".to_string(), "tool".to_string()],
                configurations: vec![],
                parameters: Parameters::new(),
                result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
                sql_tables: vec![],
                sql_queries: vec![],
                tools: None,
                oauth: None,
            },
            tool_router_key: Some(tool_router_key),
            job_id: "job_123".to_string(),
            job_id_history: vec![],
            code: "console.log('Hello, world!');".to_string(),
            assets: None,
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
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            name: name.to_string(),
            author: "Deno Author".to_string(),
            version: "1.0.0".to_string(),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![],
            config: vec![],
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            oauth: None,
            assets: None,
        };

        let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);
        let vector = SqliteManager::generate_vector_for_testing(0.1);

        // Add the tool to the database
        manager.add_tool_with_vector(shinkai_tool.clone(), vector).unwrap();

        // Return the tool_router_key generated from the DenoTool
        shinkai_tool.tool_router_key().to_string_without_version()
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
        let manager = setup_test_db().await;

        // Add a tool to ensure the tool_router_key exists
        let deno_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Test Tool".to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Deno Author".to_string(),
            version: "1.0.0".to_string(),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![],
            config: vec![],
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            oauth: None,
            assets: None,
        };

        let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);
        let vector = SqliteManager::generate_vector_for_testing(0.1);
        manager.add_tool_with_vector(shinkai_tool.clone(), vector).unwrap();

        // Create and add a ToolPlayground entry
        let tool_playground = create_test_tool_playground(shinkai_tool.tool_router_key().to_string_without_version());
        manager.set_tool_playground(&tool_playground).unwrap();

        // Add a message to the tool_playground_code_history table
        let message_id = "msg-001";
        let code = "console.log('Message Code');";
        manager
            .add_tool_playground_code_history(
                message_id,
                &shinkai_tool.tool_router_key().to_string_without_version(),
                code,
            )
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
        manager
            .remove_tool_playground(&shinkai_tool.tool_router_key().to_string_without_version())
            .unwrap();

        // Verify the ToolPlayground is removed
        let result = manager.get_tool_playground(&shinkai_tool.tool_router_key().to_string_without_version());
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
