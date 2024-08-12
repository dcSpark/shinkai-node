use crate::tools::error::ToolError;
use crate::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader};
use arrow_array::{Array, BooleanArray};
use arrow_array::{FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field};
use futures::TryStreamExt;
use lancedb::query::QueryBase;
use lancedb::query::{ExecutableQuery, Select};
use lancedb::table::AddDataMode;
use lancedb::{connect, Connection, Table};
use serde_json::Value;
use shinkai_vector_resources::model_type::EmbeddingModelType;
use std::sync::Arc;

use super::ollama_embedding_fn::OllamaEmbeddingFunction;
use super::shinkai_lancedb_error::ShinkaiLanceDBError;
use super::shinkai_tool_schema::ShinkaiToolSchema;

pub struct LanceShinkaiDb {
    #[allow(dead_code)]
    connection: Connection,
    table: Table,
    embedding_model: EmbeddingModelType,
    embedding_function: OllamaEmbeddingFunction,
}

impl LanceShinkaiDb {
    pub async fn new(db_path: &str, embedding_model: EmbeddingModelType) -> Result<Self, ShinkaiLanceDBError> {
        let connection = connect(db_path).execute().await?;
        let table = Self::create_tool_router_table(&connection, &embedding_model).await?;
        let embedding_function =
            OllamaEmbeddingFunction::new("http://localhost:11434/api/embeddings", embedding_model.clone());

        Ok(LanceShinkaiDb {
            connection,
            table,
            embedding_model,
            embedding_function,
        })
    }

    async fn create_tool_router_table(
        connection: &Connection,
        embedding_model: &EmbeddingModelType,
    ) -> Result<Table, ShinkaiLanceDBError> {
        let schema = ShinkaiToolSchema::create_schema(embedding_model)
            .map_err(|e| ShinkaiLanceDBError::Schema(e.to_string()))?;

        connection
            .create_empty_table("tool_router", schema)
            .execute()
            .await
            .map_err(ShinkaiLanceDBError::from)
    }

    pub async fn set_tool(&self, shinkai_tool: &ShinkaiTool) -> Result<(), ToolError> {
        let tool_key = shinkai_tool.tool_router_key();
        let tool_keys = vec![shinkai_tool.tool_router_key()];
        let tool_types = vec![shinkai_tool.tool_type().to_string()];

        // Check if the tool already exists and delete it if it does
        if let Some(_) = self
            .get_tool(&tool_key)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?
        {
            self.table
                .delete(format!("{} = '{}'", ShinkaiToolSchema::tool_key_field(), tool_key).as_str())
                .await
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        }

        // Get or generate the embedding
        let embedding = match shinkai_tool.get_embedding() {
            Some(embedding) => embedding.vector,
            None => {
                let embedding_string = shinkai_tool.format_embedding_string();
                self.embedding_function
                    .request_embeddings(&embedding_string)
                    .await
                    .map_err(|e| ToolError::EmbeddingGenerationError(e.to_string()))?
            }
        };
        let vectors = embedding;

        let tool_data =
            vec![serde_json::to_string(shinkai_tool).map_err(|e| ToolError::SerializationError(e.to_string()))?];

        let tool_header = vec![serde_json::to_string(&shinkai_tool.to_header())
            .map_err(|e| ToolError::SerializationError(e.to_string()))?];

        let schema = self
            .table
            .schema()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        let vector_dimensions = self
            .embedding_model
            .vector_dimensions()
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        let vectors_normalized = Arc::new(Float32Array::from(vectors));

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(tool_keys)),
                Arc::new(
                    FixedSizeListArray::try_new(
                        Arc::new(Field::new("item", DataType::Float32, true)),
                        vector_dimensions.try_into().unwrap(),
                        vectors_normalized,
                        None,
                    )
                    .map_err(|e| ToolError::DatabaseError(e.to_string()))?,
                ),
                Arc::new(StringArray::from(tool_types)),
                Arc::new(StringArray::from(tool_data)),
                Arc::new(StringArray::from(tool_header)),
                Arc::new(BooleanArray::from(vec![true])), // is_enabled is true by default
                Arc::new(StringArray::from(vec![None as Option<&str>])), // config is null by default
            ],
        )
        .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        let batch_reader = Box::new(RecordBatchIterator::new(vec![Ok(batch)], schema.clone()));

        self.table
            .add(batch_reader)
            .mode(AddDataMode::Append)
            .execute()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_tool(&self, tool_key: &str) -> Result<Option<ShinkaiTool>, ShinkaiLanceDBError> {
        let query = self
            .table
            .query()
            .only_if(format!("{} = '{}'", ShinkaiToolSchema::tool_key_field(), tool_key))
            .limit(1)
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        let mut results = query
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
        if let Some(batch) = results.pop() {
            let tool_data_array = batch
                .column_by_name(ShinkaiToolSchema::tool_data_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            if tool_data_array.len() > 0 {
                let tool_data = tool_data_array.value(0).to_string();
                let shinkai_tool: ShinkaiTool =
                    serde_json::from_str(&tool_data).map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
                return Ok(Some(shinkai_tool));
            }
        }
        Ok(None)
    }

    pub async fn remove_tool(&self, tool_key: &str) -> Result<(), ShinkaiLanceDBError> {
        self.table
            .delete(format!("{} = '{}'", ShinkaiToolSchema::tool_key_field(), tool_key).as_str())
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))
    }

    pub async fn get_all_workflows(&self) -> Result<Vec<ShinkaiToolHeader>, ShinkaiLanceDBError> {
        let query = self
            .table
            .query()
            .select(Select::columns(&[
                ShinkaiToolSchema::tool_key_field(),
                ShinkaiToolSchema::tool_type_field(),
                ShinkaiToolSchema::tool_header_field(),
            ]))
            .only_if(format!("{} = 'Workflow'", ShinkaiToolSchema::tool_type_field()))
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        let results = query
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        let mut workflows = Vec::new();
        for batch in results {
            let tool_header_array = batch
                .column_by_name(ShinkaiToolSchema::tool_header_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..tool_header_array.len() {
                let tool_header_json = tool_header_array.value(i).to_string();
                let tool_header: ShinkaiToolHeader = serde_json::from_str(&tool_header_json)
                    .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
                workflows.push(tool_header);
            }
        }
        Ok(workflows)
    }

    pub async fn set_config(&self, tool_key: &str, config: &Value) -> Result<(), ToolError> {
        // Serialize the config to a JSON string
        let config_str = serde_json::to_string(config).map_err(|e| ToolError::SerializationError(e.to_string()))?;

        // Update the tool in the database
        let filter = format!("{} = '{}'", ShinkaiToolSchema::tool_key_field(), tool_key);
        let update_builder = self
            .table
            .update()
            .only_if(filter)
            .column(ShinkaiToolSchema::config_field(), format!("'{}'", config_str));

        update_builder
            .execute()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_config(&self, tool_key: &str) -> Result<Option<Value>, ToolError> {
        let query = self
            .table
            .query()
            .select(Select::columns(&[ShinkaiToolSchema::config_field()]))
            .only_if(format!("{} = '{}'", ShinkaiToolSchema::tool_key_field(), tool_key))
            .limit(1)
            .execute()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        let results = query
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        if let Some(batch) = results.first() {
            let config_array = batch
                .column_by_name(ShinkaiToolSchema::config_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            if config_array.len() > 0 && config_array.is_valid(0) {
                let config_str = config_array.value(0);
                let config_value: Value =
                    serde_json::from_str(config_str).map_err(|e| ToolError::SerializationError(e.to_string()))?;
                return Ok(Some(config_value));
            }
        }
        Ok(None)
    }

    pub async fn vector_search(&self, query: &str, num_results: u64) -> Result<Vec<RecordBatch>, ToolError> {
        // Generate the embedding from the query string
        let embedding = self
            .embedding_function
            .request_embeddings(query)
            .await
            .map_err(|e| ToolError::EmbeddingGenerationError(e.to_string()))?;

        let query = self
            .table
            .query()
            .limit(num_results as usize)
            .nearest_to(embedding)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        let results = query
            .execute()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        let tool_data: Vec<RecordBatch> = results
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?
            .into_iter()
            .collect();

        Ok(tool_data)
    }

    // Add more methods as needed, e.g., delete_tool, get_tool, etc.
}

#[cfg(test)]
mod tests {
    use crate::tools::js_toolkit::JSToolkit;
    use crate::tools::tool_router_dep::workflows_data;

    use super::*;
    use arrow_array::Array;
    use serde_json::Value;
    use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
    use shinkai_tools_runner::built_in_tools;
    use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
    use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::time::Instant;

    fn setup() {
        let path = Path::new("lance_db_tests/");
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn test_vector_search_and_basics() -> Result<(), ShinkaiLanceDBError> {
        init_default_tracing();
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone()).await?;

        let tools = built_in_tools::get_tools();

        // Start the timer
        let start_time = Instant::now();

        // Install built-in toolkits
        let mut tool_count = 0;

        // Install built-in toolkits
        for (name, definition) in tools {
            let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
            for tool in toolkit.tools {
                let mut shinkai_tool = ShinkaiTool::JS(tool.clone());
                let embedding = generator
                    .generate_embedding_default(&shinkai_tool.format_embedding_string())
                    .await
                    .unwrap();
                shinkai_tool.set_embedding(embedding);

                db.set_tool(&shinkai_tool)
                    .await
                    .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
                tool_count += 1;
            }
        }

        // Stop the timer
        let duration = start_time.elapsed();
        println!("Added {} tools in {:?}", tool_count, duration);

        let query = "search";
        let results = db
            .vector_search(&query, 5)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        let mut found_duckduckgo = false;
        let mut duckduckgo_tool_key = String::new();

        for batch in results {
            let tool_key_array = batch
                .column_by_name(ShinkaiToolSchema::tool_key_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..tool_key_array.len() {
                let tool_key = tool_key_array.value(i);
                println!("Tool key: {}", tool_key);
                if tool_key.contains("duckduckgo") {
                    found_duckduckgo = true;
                    duckduckgo_tool_key = tool_key.to_string();
                    eprintln!("Found duckduckgo tool key: {}", duckduckgo_tool_key);
                }
            }
        }

        assert!(found_duckduckgo, "duckduckgo not found in the results");

        // Use get_tool to fetch the tool and check that they match
        let fetched_tool = db.get_tool(&duckduckgo_tool_key).await?;
        assert!(fetched_tool.is_some(), "Failed to fetch the tool using get_tool");
        let mut fetched_tool = fetched_tool.unwrap();
        assert_eq!(
            fetched_tool.tool_router_key(),
            duckduckgo_tool_key,
            "Tool keys do not match"
        );

        // Print the author name before the change
        if let ShinkaiTool::JS(ref js_tool) = fetched_tool {
            println!("Author before change: {}", js_tool.author);
        }

        // Update the author name of the tool
        if let ShinkaiTool::JS(ref mut js_tool) = fetched_tool {
            js_tool.author = "New Author".to_string();
        }

        // Update the tool in the database
        db.set_tool(&fetched_tool)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // Re-fetch the tool and check the updated author name
        let updated_tool = db.get_tool(&duckduckgo_tool_key).await?;
        assert!(updated_tool.is_some(), "Failed to fetch the updated tool");
        let updated_tool = updated_tool.unwrap();
        if let ShinkaiTool::JS(js_tool) = updated_tool {
            assert_eq!(js_tool.author, "New Author", "Author name was not updated");
        }

        // Delete the tool from the database
        db.remove_tool(&duckduckgo_tool_key)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // Try to fetch the tool again and ensure it is not available
        let deleted_tool = db.get_tool(&duckduckgo_tool_key).await?;
        assert!(deleted_tool.is_none(), "Tool was not deleted successfully");

        Ok(())
    }

    #[tokio::test]
    async fn test_add_tools_and_workflows() -> Result<(), ShinkaiLanceDBError> {
        init_default_tracing();
        setup();

        // Set the environment variable to enable testing workflows
        // env::set_var("IS_TESTING", "true");

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone()).await?;

        let tools = built_in_tools::get_tools();

        for (name, definition) in tools {
            let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
            for tool in toolkit.tools {
                let mut shinkai_tool = ShinkaiTool::JS(tool.clone());
                let embedding = generator
                    .generate_embedding_default(&shinkai_tool.format_embedding_string())
                    .await
                    .unwrap();
                shinkai_tool.set_embedding(embedding);

                db.set_tool(&shinkai_tool)
                    .await
                    .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
            }
        }

        // Start the timer
        let start_time = Instant::now();
        // Install built-in toolkits
        let mut tool_count = 0;
        // Install workflows
        let data = workflows_data::WORKFLOWS_JSON; // WORKFLOWS_JSON_TESTING
        let json_value: Value = serde_json::from_str(data).expect("Failed to parse JSON data");
        let json_array = json_value.as_array().expect("Expected JSON data to be an array");

        for item in json_array {
            let shinkai_tool: Result<ShinkaiTool, _> = serde_json::from_value(item.clone());
            let shinkai_tool = match shinkai_tool {
                Ok(tool) => tool,
                Err(e) => {
                    eprintln!("Failed to parse shinkai_tool: {}. JSON: {:?}", e, item);
                    continue; // Skip this item and continue with the next one
                }
            };

            db.set_tool(&shinkai_tool)
                .await
                .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
            tool_count += 1;
        }

        // Stop the timer
        let duration = start_time.elapsed();
        println!("Added {} workflows in {:?}", tool_count, duration);

        let query = "search";

        // Start the search timer
        let search_start_time = Instant::now();

        let results = db
            .vector_search(&query, 2)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // eprintln!("Workflow Results: {:?}", results);

        // Stop the search timer
        let search_duration = search_start_time.elapsed();
        println!("Search took {:?}", search_duration);

        assert!(!results.is_empty(), "No results found for the query");

        // Measure get_all_workflows execution time
        let workflows_start_time = Instant::now();

        let workflows = db.get_all_workflows().await?;
        // eprintln!("Workflows: {:?}", workflows);
        let workflows_duration = workflows_start_time.elapsed();
        println!("get_all_workflows took {:?}", workflows_duration);

        assert!(!workflows.is_empty(), "No workflows found");

        Ok(())
    }

    #[tokio::test]
    async fn test_add_tool_and_update_config() -> Result<(), ShinkaiLanceDBError> {
        init_default_tracing();
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone()).await?;

        let tools = built_in_tools::get_tools();
        let (name, definition) = tools.into_iter().next().unwrap();
        let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
        let tool = toolkit.tools.into_iter().next().unwrap();
        let mut shinkai_tool = ShinkaiTool::JS(tool.clone());
        let embedding = generator
            .generate_embedding_default(&shinkai_tool.format_embedding_string())
            .await
            .unwrap();
        shinkai_tool.set_embedding(embedding);

        db.set_tool(&shinkai_tool)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // Read the config (should be null)
        let tool_key = shinkai_tool.tool_router_key();
        let config = db
            .get_config(&tool_key)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
        println!("Initial config: {:?}", config);
        assert!(config.is_none(), "Config should be null initially");

        // Update the config with a random JSON value
        let new_config: Value = serde_json::json!({"key": "value"});
        db.set_config(&tool_key, &new_config)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

        // Read the config back and ensure it matches
        let updated_config = db
            .get_config(&tool_key)
            .await
            .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;
        println!("Updated config: {:?}", updated_config);
        assert_eq!(updated_config, Some(new_config), "Config values do not match");

        Ok(())
    }
}
