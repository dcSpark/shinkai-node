use super::shinkai_prompt_schema::ShinkaiPromptSchema;
use super::{shinkai_lance_db::LanceShinkaiDb, shinkai_lancedb_error::ShinkaiLanceDBError};
use arrow_array::{BooleanArray, RecordBatch, RecordBatchIterator, StringArray};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::{table::AddDataMode, Connection, Error as LanceDbError, Table};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use arrow_array::Array;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomPrompt {
    pub name: String,
    pub prompt: String,
    pub is_system: bool,
    pub is_enabled: bool,
    pub version: String,
    pub is_favorite: bool,
}

impl LanceShinkaiDb {
    pub async fn create_prompt_table(connection: &Connection) -> Result<Table, ShinkaiLanceDBError> {
        let schema = ShinkaiPromptSchema::create_schema().map_err(ShinkaiLanceDBError::from)?;

        match connection.create_empty_table("prompts", schema).execute().await {
            Ok(table) => Ok(table),
            Err(LanceDbError::TableAlreadyExists { .. }) => connection
                .open_table("prompts")
                .execute()
                .await
                .map_err(ShinkaiLanceDBError::from),
            Err(e) => Err(ShinkaiLanceDBError::from(e)),
        }
    }

    pub async fn get_prompt(&self, name: &str) -> Result<Option<CustomPrompt>, ShinkaiLanceDBError> {
        let query = self
            .prompt_table
            .query()
            .select(Select::columns(&[
                ShinkaiPromptSchema::name_field(),
                ShinkaiPromptSchema::prompt_field(),
                ShinkaiPromptSchema::is_system_field(),
                ShinkaiPromptSchema::is_enabled_field(),
                ShinkaiPromptSchema::version_field(),
                ShinkaiPromptSchema::is_favorite_field(),
            ]))
            .only_if(format!("{} = '{}'", ShinkaiPromptSchema::name_field(), name))
            .limit(1)
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;

        let results = query
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;

        for batch in results {
            let name_array = batch
                .column_by_name(ShinkaiPromptSchema::name_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let prompt_array = batch
                .column_by_name(ShinkaiPromptSchema::prompt_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let is_system_array = batch
                .column_by_name(ShinkaiPromptSchema::is_system_field())
                .unwrap()
                .as_any()
                .downcast_ref::<BooleanArray>()
                .unwrap();
            let is_enabled_array = batch
                .column_by_name(ShinkaiPromptSchema::is_enabled_field())
                .unwrap()
                .as_any()
                .downcast_ref::<BooleanArray>()
                .unwrap();
            let version_array = batch
                .column_by_name(ShinkaiPromptSchema::version_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let is_favorite_array = batch
                .column_by_name(ShinkaiPromptSchema::is_favorite_field())
                .unwrap()
                .as_any()
                .downcast_ref::<BooleanArray>()
                .unwrap();

            if name_array.len() > 0 {
                return Ok(Some(CustomPrompt {
                    name: name_array.value(0).to_string(),
                    prompt: prompt_array.value(0).to_string(),
                    is_system: is_system_array.value(0),
                    is_enabled: is_enabled_array.value(0),
                    version: version_array.value(0).to_string(),
                    is_favorite: is_favorite_array.value(0),
                }));
            }
        }
        Ok(None)
    }

    pub async fn set_prompt(&self, prompt: CustomPrompt) -> Result<(), ShinkaiLanceDBError> {
        let schema = self.prompt_table.schema().await.map_err(ShinkaiLanceDBError::from)?;
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![prompt.name])),
                Arc::new(BooleanArray::from(vec![prompt.is_system])),
                Arc::new(BooleanArray::from(vec![prompt.is_enabled])),
                Arc::new(StringArray::from(vec![prompt.version])),
                Arc::new(StringArray::from(vec![prompt.prompt])),
                Arc::new(BooleanArray::from(vec![prompt.is_favorite])),
            ],
        )
        .map_err(|e| ShinkaiLanceDBError::Arrow(e.to_string()))?;
        let batch_reader = Box::new(RecordBatchIterator::new(vec![Ok(batch)], schema.clone()));
        self.prompt_table
            .add(batch_reader)
            .mode(AddDataMode::Append)
            .execute()
            .await
            .map_err(ShinkaiLanceDBError::from)?;
        Ok(())
    }

    pub async fn prompt_vector_search(
        &self,
        query: &str,
        num_results: u64,
    ) -> Result<Vec<String>, ShinkaiLanceDBError> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // Generate the embedding from the query string
        let embedding = self
            .embedding_function
            .request_embeddings(query)
            .await
            .map_err(|e| ShinkaiLanceDBError::EmbeddingGenerationError(e.to_string()))?;

        let query = self
            .prompt_table
            .query()
            .select(Select::columns(&[
                ShinkaiPromptSchema::name_field(),
                ShinkaiPromptSchema::prompt_field(),
            ]))
            .limit(num_results as usize)
            .nearest_to(embedding)
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;

        let results = query
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;

        let mut prompts = Vec::new();
        let batches = results
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;

        for batch in batches {
            let prompt_array = batch
                .column_by_name(ShinkaiPromptSchema::prompt_field())
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..prompt_array.len() {
                let prompt = prompt_array.value(i).to_string();
                prompts.push(prompt);
            }
        }

        Ok(prompts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
    use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
    use std::fs;
    use std::path::Path;

    fn setup() {
        let path = Path::new("lance_db_tests/");
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn test_prompt_management() -> Result<(), ShinkaiLanceDBError> {
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?;

        let test_prompt = CustomPrompt {
            name: "test_prompt".to_string(),
            prompt: "This is a test prompt".to_string(),
            is_system: true,
            is_enabled: true,
            version: "1".to_string(),
            is_favorite: true,
        };

        // Set a prompt
        db.set_prompt(test_prompt.clone()).await?;

        // Get the prompt
        let prompt = db.get_prompt("test_prompt").await?;
        assert_eq!(prompt, Some(test_prompt), "Prompt should match");

        Ok(())
    }
}