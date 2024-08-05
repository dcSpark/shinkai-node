use crate::tools::error::ToolError;
use crate::tools::shinkai_tool::ShinkaiTool;
use arrow_array::{FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use lancedb::query::ExecutableQuery;
use lancedb::query::QueryBase;
use lancedb::table::AddDataMode;
use lancedb::Error as LanceDbError;
use lancedb::{connect, Connection, Result as LanceDbResult, Table};
use shinkai_vector_resources::{embeddings::Embedding, model_type::EmbeddingModelType};
use std::sync::Arc;

use super::ollama_embedding_fn::OllamaEmbeddingFunction;
use super::shinkai_lancedb_error::ShinkaiLanceDBError;
use super::shinkai_tool_schema::ShinkaiToolSchema;

pub struct LanceShinkaiDb {
    connection: Connection,
    table: Table,
    embedding_model: EmbeddingModelType,
    embedding_function: OllamaEmbeddingFunction,
}

impl LanceShinkaiDb {
    pub async fn new(embedding_model: EmbeddingModelType) -> Result<Self, ShinkaiLanceDBError> {
        let uri = "db/lancedb";
        let connection = connect(uri).execute().await?;
        let table = Self::create_tool_router_table(&connection, &embedding_model).await?;
        let embedding_function = OllamaEmbeddingFunction::new("http://localhost:11434/api/embeddings", embedding_model.clone());

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
        let schema = ShinkaiToolSchema::create_schema(embedding_model).map_err(|e| ShinkaiLanceDBError::Schema(e.to_string()))?;

        connection
            .create_empty_table("tool_router", schema)
            .execute()
            .await
            .map_err(ShinkaiLanceDBError::from)
    }

    pub async fn insert_tools(&self, profile: &str, shinkai_tools: &[ShinkaiTool]) -> Result<(), ToolError> {
        let mut profiles = Vec::new();
        let mut tool_keys = Vec::new();
        let mut tool_types = Vec::new();
        let mut embedding_models = Vec::new();
        let mut vectors = Vec::new();
        let mut tool_data = Vec::new();

        for shinkai_tool in shinkai_tools {
            profiles.push(profile.to_string());
            tool_keys.push(shinkai_tool.tool_router_key());
            tool_types.push(shinkai_tool.tool_type());
            embedding_models.push(self.embedding_model.to_string());

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
            vectors.extend(embedding);

            tool_data
                .push(serde_json::to_string(shinkai_tool).map_err(|e| ToolError::SerializationError(e.to_string()))?);
        }

        let schema = self
            .table
            .schema()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        let vector_dimensions = self
            .embedding_model
            .vector_dimensions()
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(profiles)),
                Arc::new(StringArray::from(tool_keys)),
                Arc::new(StringArray::from(tool_types)),
                Arc::new(StringArray::from(embedding_models)),
                Arc::new(
                    FixedSizeListArray::try_new(
                        Arc::new(Field::new("item", DataType::Float32, true)),
                        vector_dimensions.try_into().unwrap(),
                        Arc::new(Float32Array::from(vectors)),
                        None,
                    )
                    .map_err(|e| ToolError::DatabaseError(e.to_string()))?,
                ),
                Arc::new(StringArray::from(tool_data)),
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

    pub async fn vector_search(
        &self,
        profile: &str,
        query: &str,
        num_results: u64,
    ) -> Result<Vec<RecordBatch>, ToolError> {
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
            .filter(|row| {
                let profile_field = row[ShinkaiToolSchema::profile_field()]
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                profile_field.value(0) == profile
            })
            .collect();

        Ok(tool_data)
    }

    // Add more methods as needed, e.g., delete_tool, get_tool, etc.
}
