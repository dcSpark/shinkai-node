use super::shinkai_prompt_schema::ShinkaiPromptSchema;
use super::{shinkai_lance_db::LanceShinkaiDb, shinkai_lancedb_error::ShinkaiLanceDBError};
use arrow_array::{Array, FixedSizeListArray, Float32Array};
use arrow_array::{BooleanArray, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field};
use futures::{StreamExt, TryStreamExt};
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::{table::AddDataMode, Connection, Error as LanceDbError, Table};
use serde::{Deserialize, Serialize};
use shinkai_vector_resources::model_type::EmbeddingModelType;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomPrompt {
    pub name: String,
    pub prompt: String,
    pub is_system: bool,
    pub is_enabled: bool,
    pub version: String,
    pub is_favorite: bool,
    pub embedding: Option<Vec<f32>>,
}

impl CustomPrompt {
    pub fn text_for_embedding(&self) -> String {
        format!("{} {}", self.name, self.prompt)
    }
}

impl LanceShinkaiDb {
    pub async fn create_prompt_table(
        connection: &Connection,
        embedding_model: &EmbeddingModelType,
    ) -> Result<Table, ShinkaiLanceDBError> {
        let schema = ShinkaiPromptSchema::create_schema(embedding_model).map_err(ShinkaiLanceDBError::from)?;

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

    fn convert_batch_to_prompt(batch: &RecordBatch) -> Option<CustomPrompt> {
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
        let vector_array = batch
            .column_by_name(ShinkaiPromptSchema::vector_field())
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .unwrap();

        if name_array.len() > 0 {
            let embedding = if vector_array.is_null(0) {
                None
            } else {
                Some(
                    vector_array
                        .value(0)
                        .as_any()
                        .downcast_ref::<Float32Array>()
                        .unwrap()
                        .values()
                        .to_vec(),
                )
            };

            Some(CustomPrompt {
                name: name_array.value(0).to_string(),
                prompt: prompt_array.value(0).to_string(),
                is_system: is_system_array.value(0),
                is_enabled: is_enabled_array.value(0),
                version: version_array.value(0).to_string(),
                is_favorite: is_favorite_array.value(0),
                embedding,
            })
        } else {
            None
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
                ShinkaiPromptSchema::vector_field(),
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
            if let Some(prompt) = Self::convert_batch_to_prompt(&batch) {
                return Ok(Some(prompt));
            }
        }
        Ok(None)
    }

    pub async fn set_prompt(&self, prompt: CustomPrompt) -> Result<(), ShinkaiLanceDBError> {
        let schema = self.prompt_table.schema().await.map_err(ShinkaiLanceDBError::from)?;

        // Check if the prompt already exists and delete it if it does
        if self.get_prompt(&prompt.name).await?.is_some() {
            eprintln!("Prompt already exists, deleting it: {}", prompt.name);
            self.prompt_table
                .delete(format!("{} = '{}'", ShinkaiPromptSchema::name_field(), prompt.name).as_str())
                .await
                .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;
        }

        // Get or generate the embedding
        let vectors = match prompt.embedding {
            Some(embedding) => embedding,
            None => {
                eprintln!("Generating embedding for prompt: {}", prompt.name);
                let embedding_string = prompt.text_for_embedding();
                self.embedding_function
                    .request_embeddings(&embedding_string)
                    .await
                    .map_err(|e| ShinkaiLanceDBError::EmbeddingGenerationError(e.to_string()))?
            }
        };

        let vector_dimensions = self
            .embedding_model
            .vector_dimensions()
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;

        let vectors_normalized = Arc::new(Float32Array::from(vectors));

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![prompt.name])),
                Arc::new(BooleanArray::from(vec![prompt.is_system])),
                Arc::new(BooleanArray::from(vec![prompt.is_enabled])),
                Arc::new(StringArray::from(vec![prompt.version])),
                Arc::new(StringArray::from(vec![prompt.prompt])),
                Arc::new(BooleanArray::from(vec![prompt.is_favorite])),
                Arc::new(
                    FixedSizeListArray::try_new(
                        Arc::new(Field::new("item", DataType::Float32, true)),
                        vector_dimensions.try_into().unwrap(),
                        vectors_normalized,
                        None,
                    )
                    .map_err(|e| ShinkaiLanceDBError::Arrow(e.to_string()))?,
                ),
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

    pub async fn get_favorite_prompts(&self) -> Result<Vec<CustomPrompt>, ShinkaiLanceDBError> {
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
                ShinkaiPromptSchema::vector_field(),
            ]))
            .only_if(format!("{} = true", ShinkaiPromptSchema::is_favorite_field()))
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;

        let mut prompts = Vec::new();
        let mut res = query;
        while let Some(Ok(batch)) = res.next().await {
            for _i in 0..batch.num_rows() {
                if let Some(prompt) = Self::convert_batch_to_prompt(&batch) {
                    prompts.push(prompt);
                }
            }
        }

        Ok(prompts)
    }

    pub async fn prompt_vector_search(
        &self,
        query: &str,
        num_results: u64,
    ) -> Result<Vec<CustomPrompt>, ShinkaiLanceDBError> {
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
                ShinkaiPromptSchema::is_system_field(),
                ShinkaiPromptSchema::is_enabled_field(),
                ShinkaiPromptSchema::version_field(),
                ShinkaiPromptSchema::is_favorite_field(),
                ShinkaiPromptSchema::vector_field(),
            ]))
            .limit(num_results as usize)
            .nearest_to(embedding)
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;

        let mut prompts = Vec::new();
        let mut results = query
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;

        while let Some(Ok(batch)) = results.next().await {
            if let Some(prompt) = Self::convert_batch_to_prompt(&batch) {
                prompts.push(prompt);
            }
        }

        Ok(prompts)
    }

    pub async fn get_all_prompts(&self) -> Result<Vec<CustomPrompt>, ShinkaiLanceDBError> {
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
                ShinkaiPromptSchema::vector_field(),
            ]))
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;

        let mut prompts = Vec::new();
        let mut res = query;
        while let Some(Ok(batch)) = res.next().await {
            for _i in 0..batch.num_rows() {
                if let Some(prompt) = Self::convert_batch_to_prompt(&batch) {
                    prompts.push(prompt);
                }
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
            embedding: None,
        };

        // Set a prompt
        db.set_prompt(test_prompt.clone()).await?;

        // Get the prompt
        let prompt = db.get_prompt("test_prompt").await?;
        assert_eq!(prompt, Some(test_prompt), "Prompt should match");

        Ok(())
    }

    #[tokio::test]
    async fn test_add_and_get_all_prompts() -> Result<(), ShinkaiLanceDBError> {
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?;

        let prompts = vec![
            CustomPrompt {
                name: "prompt1".to_string(),
                prompt: "This is the first test prompt".to_string(),
                is_system: false,
                is_enabled: true,
                version: "1".to_string(),
                is_favorite: false,
                embedding: None,
            },
            CustomPrompt {
                name: "prompt2".to_string(),
                prompt: "This is the second test prompt".to_string(),
                is_system: false,
                is_enabled: true,
                version: "1".to_string(),
                is_favorite: false,
                embedding: None,
            },
            CustomPrompt {
                name: "prompt3".to_string(),
                prompt: "This is the third test prompt".to_string(),
                is_system: false,
                is_enabled: true,
                version: "1".to_string(),
                is_favorite: false,
                embedding: None,
            },
        ];

        // Set the prompts
        for prompt in &prompts {
            db.set_prompt(prompt.clone()).await?;
        }

        // Get all prompts
        let all_prompts = db.get_all_prompts().await?;
        assert_eq!(all_prompts.len(), 3, "There should be 3 prompts");

        for prompt in prompts {
            assert!(all_prompts.contains(&prompt), "Prompt should be in the list");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_add_and_update_prompt() -> Result<(), ShinkaiLanceDBError> {
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?;

        let initial_prompt = CustomPrompt {
            name: "update_test_prompt".to_string(),
            prompt: "This is a test prompt to be updated".to_string(),
            is_system: false,
            is_enabled: true,
            version: "1".to_string(),
            is_favorite: false,
            embedding: None,
        };

        // Set the initial prompt
        db.set_prompt(initial_prompt.clone()).await?;

        // Update the prompt to be a favorite
        let updated_prompt = CustomPrompt {
            is_favorite: true,
            ..initial_prompt.clone()
        };
        db.set_prompt(updated_prompt.clone()).await?;

        // Get the updated prompt
        let prompt = db.get_prompt("update_test_prompt").await?;
        assert_eq!(prompt, Some(updated_prompt), "Prompt should be updated to favorite");

        Ok(())
    }

    #[tokio::test]
    async fn test_prompt_vector_search() -> Result<(), ShinkaiLanceDBError> {
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let db = LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?;

        let prompts = vec![
            CustomPrompt {
                name: "prompt1".to_string(),
                prompt: "This is the first test prompt".to_string(),
                is_system: false,
                is_enabled: true,
                version: "1".to_string(),
                is_favorite: false,
                embedding: None,
            },
            CustomPrompt {
                name: "prompt2".to_string(),
                prompt: "This is the second test prompt".to_string(),
                is_system: false,
                is_enabled: true,
                version: "1".to_string(),
                is_favorite: false,
                embedding: None,
            },
            CustomPrompt {
                name: "prompt3".to_string(),
                prompt: "This is the third test prompt".to_string(),
                is_system: false,
                is_enabled: true,
                version: "1".to_string(),
                is_favorite: false,
                embedding: None,
            },
        ];

        // Set the prompts with embeddings
        for mut prompt in prompts.clone() {
            let embedding = generator
                .generate_embedding_default(&prompt.text_for_embedding())
                .await
                .unwrap();
            prompt.embedding = Some(embedding.vector);
            db.set_prompt(prompt).await?;
        }

        // Get all prompts
        let all_prompts = db.get_all_prompts().await?;
        assert_eq!(all_prompts.len(), 3, "There should be 3 prompts");

        // Perform a vector search
        let search_query = "first test prompt";
        let search_results = db.prompt_vector_search(search_query, 2).await?;
        assert_eq!(search_results.len(), 1, "There should be 1 search result");

        Ok(())
    }
}
