use crate::prompts::custom_prompt::CustomPrompt;

use super::shinkai_prompt_schema::ShinkaiPromptSchema;
use super::{shinkai_lance_db::LanceShinkaiDb, shinkai_lancedb_error::ShinkaiLanceDBError};
use arrow_array::{Array, FixedSizeListArray, Float32Array};
use arrow_array::{BooleanArray, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field};
use futures::{StreamExt, TryStreamExt};
use lancedb::connection::LanceFileVersion;
use lancedb::index::scalar::{FtsIndexBuilder, FullTextSearchQuery};
use lancedb::index::vector::IvfHnswPqIndexBuilder;
use lancedb::index::Index;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::{table::AddDataMode, Connection, Error as LanceDbError, Table};
use shinkai_vector_resources::model_type::EmbeddingModelType;
use std::collections::HashSet;
use std::sync::Arc;

impl LanceShinkaiDb {
    pub async fn create_prompt_table(
        connection: &Connection,
        embedding_model: &EmbeddingModelType,
    ) -> Result<Table, ShinkaiLanceDBError> {
        let schema = ShinkaiPromptSchema::create_schema(embedding_model).map_err(ShinkaiLanceDBError::from)?;

        let table = match connection
            .create_empty_table("prompts_v2", schema)
            .data_storage_version(LanceFileVersion::V2_1)
            .enable_v2_manifest_paths(true)
            .execute()
            .await
        {
            Ok(table) => table,
            Err(e) => {
                if let LanceDbError::TableAlreadyExists { .. } = e {
                    connection
                        .open_table("prompts_v2")
                        .execute()
                        .await
                        .map_err(ShinkaiLanceDBError::from)?
                } else {
                    return Err(ShinkaiLanceDBError::from(e));
                }
            }
        };

        Ok(table)
    }

    pub async fn create_prompt_indices_if_needed(&self) -> Result<(), ShinkaiLanceDBError> {
        // Check if the table is empty
        let is_empty = Self::is_table_empty(&self.prompt_table).await?;
        if is_empty {
            eprintln!("Prompt Table is empty, skipping index creation.");
            return Ok(());
        }

        // Check the number of elements in the table
        let element_count = self.prompt_table.count_rows(None).await?;
        if element_count < 100 {
            self.prompt_table
                .create_index(&["prompt"], Index::FTS(FtsIndexBuilder::default()))
                .execute()
                .await?;

            eprintln!("Not enough elements to create other indices. Skipping index creation for prompt table.");
            return Ok(());
        }

        // Create the indices
        self.prompt_table.create_index(&["name"], Index::Auto).execute().await?;

        self.prompt_table
            .create_index(&["prompt"], Index::Auto)
            .execute()
            .await?;

        self.prompt_table
            .create_index(&["vector"], Index::Auto)
            .execute()
            .await?;
        self.prompt_table
            .create_index(
                &[ShinkaiPromptSchema::vector_field()],
                Index::IvfHnswPq(IvfHnswPqIndexBuilder::default()),
            )
            .execute()
            .await?;

        Ok(())
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

        // Full-text search query
        let fts_query_builder = self
            .prompt_table
            .query()
            .full_text_search(FullTextSearchQuery::new(query.to_owned()))
            .select(Select::columns(&[
                ShinkaiPromptSchema::name_field(),
                ShinkaiPromptSchema::prompt_field(),
                ShinkaiPromptSchema::is_system_field(),
                ShinkaiPromptSchema::is_enabled_field(),
                ShinkaiPromptSchema::version_field(),
                ShinkaiPromptSchema::is_favorite_field(),
                ShinkaiPromptSchema::vector_field(),
            ]))
            .limit(num_results as usize);

        // Vector search query
        let vector_query_builder = self
            .prompt_table
            .query()
            .nearest_to(embedding)
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?
            .select(Select::columns(&[
                ShinkaiPromptSchema::name_field(),
                ShinkaiPromptSchema::prompt_field(),
                ShinkaiPromptSchema::is_system_field(),
                ShinkaiPromptSchema::is_enabled_field(),
                ShinkaiPromptSchema::version_field(),
                ShinkaiPromptSchema::is_favorite_field(),
                ShinkaiPromptSchema::vector_field(),
            ]))
            .limit(num_results as usize);

        // Execute the full-text search
        let fts_query = fts_query_builder
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;
        // Execute the vector search
        let vector_query = vector_query_builder
            .execute()
            .await
            .map_err(|e| ShinkaiLanceDBError::DatabaseError(e.to_string()))?;

        // Collect results from both queries
        let mut fts_results = Vec::new();
        let mut vector_results = Vec::new();

        let mut fts_res = fts_query;
        while let Some(Ok(batch)) = fts_res.next().await {
            if let Some(prompt) = Self::convert_batch_to_prompt(&batch) {
                fts_results.push(prompt);
            }
        }

        let mut vector_res = vector_query;
        while let Some(Ok(batch)) = vector_res.next().await {
            if let Some(prompt) = Self::convert_batch_to_prompt(&batch) {
                vector_results.push(prompt);
            }
        }

        // Merge results using interleave and remove duplicates
        let mut combined_results = Vec::new();
        let mut seen = HashSet::new();
        let mut fts_iter = fts_results.into_iter();
        let mut vector_iter = vector_results.into_iter();

        while combined_results.len() < num_results as usize {
            match (fts_iter.next(), vector_iter.next()) {
                (Some(fts_item), Some(vector_item)) => {
                    if seen.insert(fts_item.name.clone()) && combined_results.len() < num_results as usize {
                        combined_results.push(fts_item);
                    }
                    if seen.insert(vector_item.name.clone()) && combined_results.len() < num_results as usize {
                        combined_results.push(vector_item);
                    }
                }
                (Some(fts_item), None) => {
                    if seen.insert(fts_item.name.clone()) && combined_results.len() < num_results as usize {
                        combined_results.push(fts_item);
                    }
                }
                (None, Some(vector_item)) => {
                    if seen.insert(vector_item.name.clone()) && combined_results.len() < num_results as usize {
                        combined_results.push(vector_item);
                    }
                }
                (None, None) => break,
            }
        }

        Ok(combined_results)
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

    pub async fn remove_prompt(&self, name: &str) -> Result<(), ShinkaiLanceDBError> {
        let delete_condition = format!("{} = '{}'", ShinkaiPromptSchema::name_field(), name);

        match self.prompt_table.delete(&delete_condition).await {
            Ok(_) => Ok(()),
            Err(e) => Err(ShinkaiLanceDBError::DatabaseError(format!(
                "Failed to remove prompt: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::prompts_static_texts::{
        AGILITY_STORY_SYSTEM, AI_SYSTEM, ANALYZE_ANSWERS_SYSTEM, ANALYZE_CLAIMS_SYSTEM, ANALYZE_DEBATE_SYSTEM,
        ANALYZE_INCIDENT_SYSTEM, ANALYZE_LOGS_SYSTEM, ANALYZE_MALWARE_SYSTEM, ANALYZE_PAPER_SYSTEM,
        ANALYZE_PATENT_SYSTEM, ANALYZE_PERSONALITY_SYSTEM, ANALYZE_PRESENTATION_SYSTEM, ANALYZE_PROSE_JSON_SYSTEM,
        ANALYZE_PROSE_PINKER_SYSTEM, ANALYZE_PROSE_SYSTEM, ANALYZE_SPIRITUAL_TEXT_SYSTEM, ANALYZE_TECH_IMPACT_SYSTEM,
        ANALYZE_THREAT_REPORT_SYSTEM, ANALYZE_THREAT_REPORT_TRENDS_SYSTEM, ANALYZE_THREAT_REPORT_TRENDS_USER,
        ANALYZE_THREAT_REPORT_USER, ANSWER_INTERVIEW_QUESTION_SYSTEM, ASK_SECURE_BY_DESIGN_QUESTIONS_SYSTEM,
        CAPTURE_THINKERS_WORK_SYSTEM, CHECK_AGREEMENT_SYSTEM, CLEAN_TEXT_SYSTEM, CODING_MASTER_SYSTEM,
        COMPARE_AND_CONTRAST_SYSTEM, CREATE_5_SENTENCE_SUMMARY_SYSTEM, CREATE_ACADEMIC_PAPER_SYSTEM,
        CREATE_AI_JOBS_ANALYSIS_SYSTEM, CREATE_APHORISMS_SYSTEM, CREATE_ART_PROMPT_SYSTEM, CREATE_BETTER_FRAME_SYSTEM,
        CREATE_CODING_PROJECT_SYSTEM, CREATE_COMMAND_SYSTEM, CREATE_CYBER_SUMMARY_SYSTEM,
        CREATE_GIT_DIFF_COMMIT_SYSTEM, CREATE_GRAPH_FROM_INPUT_SYSTEM, CREATE_HORMOZI_OFFER_SYSTEM,
        CREATE_IDEA_COMPASS_SYSTEM, CREATE_INVESTIGATION_VISUALIZATION_SYSTEM, CREATE_KEYNOTE_SYSTEM,
        CREATE_LOGO_SYSTEM, CREATE_MARKMAP_VISUALIZATION_SYSTEM, CREATE_MERMAID_VISUALIZATION_SYSTEM,
        CREATE_MICRO_SUMMARY_SYSTEM, CREATE_NETWORK_THREAT_LANDSCAPE_SYSTEM, CREATE_NETWORK_THREAT_LANDSCAPE_USER,
        CREATE_NPC_SYSTEM, CREATE_PATTERN_SYSTEM, CREATE_QUIZ_SYSTEM, CREATE_READING_PLAN_SYSTEM,
        CREATE_REPORT_FINDING_SYSTEM, CREATE_REPORT_FINDING_USER, CREATE_SECURITY_UPDATE_SYSTEM,
        CREATE_SHOW_INTRO_SYSTEM, CREATE_SIGMA_RULES_SYSTEM, CREATE_STRIDE_THREAT_MODEL_SYSTEM, CREATE_SUMMARY_SYSTEM,
        CREATE_TAGS_SYSTEM, CREATE_THREAT_SCENARIOS_SYSTEM, CREATE_UPGRADE_PACK_SYSTEM, CREATE_VIDEO_CHAPTERS_SYSTEM,
        CREATE_VISUALIZATION_SYSTEM, EXPLAIN_CODE_SYSTEM, EXPLAIN_CODE_USER, EXPLAIN_DOCS_SYSTEM,
        EXPLAIN_PROJECT_SYSTEM, EXPLAIN_TERMS_SYSTEM, EXPORT_DATA_AS_CSV_SYSTEM,
        EXTRACT_ALGORITHM_UPDATE_RECOMMENDATIONS_SYSTEM, EXTRACT_ARTICLE_WISDOM_SYSTEM, EXTRACT_ARTICLE_WISDOM_USER,
        EXTRACT_BOOK_IDEAS_SYSTEM, EXTRACT_BOOK_RECOMMENDATIONS_SYSTEM, EXTRACT_BUSINESS_IDEAS_SYSTEM,
        EXTRACT_CONTROVERSIAL_IDEAS_SYSTEM, EXTRACT_EXTRAORDINARY_CLAIMS_SYSTEM, EXTRACT_IDEAS_SYSTEM,
        EXTRACT_INSIGHTS_SYSTEM, EXTRACT_MAIN_IDEA_SYSTEM, EXTRACT_PATTERNS_SYSTEM, EXTRACT_POC_SYSTEM,
        EXTRACT_PREDICTIONS_SYSTEM, EXTRACT_QUESTIONS_SYSTEM, EXTRACT_RECOMMENDATIONS_SYSTEM,
        EXTRACT_REFERENCES_SYSTEM, EXTRACT_SONG_MEANING_SYSTEM, EXTRACT_SPONSORS_SYSTEM, EXTRACT_VIDEOID_SYSTEM,
        EXTRACT_WISDOM_AGENTS_SYSTEM, EXTRACT_WISDOM_DM_SYSTEM, EXTRACT_WISDOM_NOMETA_SYSTEM, EXTRACT_WISDOM_SYSTEM,
        FIND_HIDDEN_MESSAGE_SYSTEM, FIND_LOGICAL_FALLACIES_SYSTEM, GENERATE_QUIZ_SYSTEM, GET_WOW_PER_MINUTE_SYSTEM,
        GET_YOUTUBE_RSS_SYSTEM, IMPROVE_ACADEMIC_WRITING_SYSTEM, IMPROVE_PROMPT_SYSTEM, IMPROVE_REPORT_FINDING_SYSTEM,
        IMPROVE_REPORT_FINDING_USER, IMPROVE_WRITING_SYSTEM, LABEL_AND_RATE_SYSTEM, OFFICIAL_PATTERN_TEMPLATE_SYSTEM,
        PROVIDE_GUIDANCE_SYSTEM, RATE_AI_RESPONSE_SYSTEM, RATE_AI_RESULT_SYSTEM, RATE_CONTENT_SYSTEM,
        RATE_CONTENT_USER, RATE_VALUE_SYSTEM, RAW_QUERY_SYSTEM, RECOMMEND_ARTISTS_SYSTEM,
        SHOW_FABRIC_OPTIONS_MARKMAP_SYSTEM, SUGGEST_PATTERN_SYSTEM, SUGGEST_PATTERN_USER, SUMMARIZE_DEBATE_SYSTEM,
        SUMMARIZE_GIT_CHANGES_SYSTEM, SUMMARIZE_GIT_DIFF_SYSTEM, SUMMARIZE_LECTURE_SYSTEM,
        SUMMARIZE_LEGISLATION_SYSTEM, SUMMARIZE_MICRO_SYSTEM, SUMMARIZE_NEWSLETTER_SYSTEM, SUMMARIZE_PAPER_SYSTEM,
        SUMMARIZE_PROMPT_SYSTEM, SUMMARIZE_PULL_REQUESTS_SYSTEM, SUMMARIZE_RPG_SESSION_SYSTEM, SUMMARIZE_SYSTEM,
        TO_FLASHCARDS_SYSTEM, TWEET_SYSTEM, WRITE_ESSAY_SYSTEM, WRITE_HACKERONE_REPORT_SYSTEM,
        WRITE_MICRO_ESSAY_SYSTEM, WRITE_PULL_REQUEST_SYSTEM, WRITE_SEMGREP_RULE_SYSTEM,
    };
    use serde_json::json;
    use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
    use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
    use std::env;
    use std::fs;
    use std::fs::File;
    use std::io::Write;
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

        // Generate embedding
        let embedding = generator
            .generate_embedding_default(&test_prompt.text_for_embedding())
            .await
            .unwrap()
            .vector;

        // Get the prompt
        let prompt = db.get_prompt("test_prompt").await?;

        // Compare the generated embedding with the one from the database
        assert_eq!(
            prompt.as_ref().unwrap().embedding,
            Some(embedding),
            "Embeddings should match"
        );

        // Compare the rest of the prompt fields
        assert_eq!(prompt.as_ref().unwrap().name, test_prompt.name, "Name should match");
        assert_eq!(
            prompt.as_ref().unwrap().prompt,
            test_prompt.prompt,
            "Prompt text should match"
        );
        assert_eq!(
            prompt.as_ref().unwrap().is_system,
            test_prompt.is_system,
            "Is system flag should match"
        );
        assert_eq!(
            prompt.as_ref().unwrap().is_enabled,
            test_prompt.is_enabled,
            "Is enabled flag should match"
        );
        assert_eq!(
            prompt.as_ref().unwrap().version,
            test_prompt.version,
            "Version should match"
        );
        assert_eq!(
            prompt.as_ref().unwrap().is_favorite,
            test_prompt.is_favorite,
            "Is favorite flag should match"
        );

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

        // Check that all prompt names are present in the retrieved prompts
        let prompt_names: Vec<String> = prompts.iter().map(|p| p.name.clone()).collect();
        for prompt in all_prompts {
            assert!(
                prompt_names.contains(&prompt.name),
                "Prompt name '{}' should be in the list",
                prompt.name
            );
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

        // Check specific fields
        assert!(prompt.is_some(), "Prompt should exist");
        let retrieved_prompt = prompt.unwrap();
        assert_eq!(retrieved_prompt.name, updated_prompt.name, "Name should match");
        assert_eq!(
            retrieved_prompt.prompt, updated_prompt.prompt,
            "Prompt text should match"
        );
        assert_eq!(
            retrieved_prompt.is_system, updated_prompt.is_system,
            "Is system flag should match"
        );
        assert_eq!(
            retrieved_prompt.is_enabled, updated_prompt.is_enabled,
            "Is enabled flag should match"
        );
        assert_eq!(retrieved_prompt.version, updated_prompt.version, "Version should match");
        assert_eq!(
            retrieved_prompt.is_favorite, updated_prompt.is_favorite,
            "Is favorite flag should match"
        );
        assert!(retrieved_prompt.embedding.is_some(), "Embedding should be present");

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

        // Create the index
        db.create_prompt_indices_if_needed().await?;

        // Get all prompts
        let all_prompts = db.get_all_prompts().await?;
        assert_eq!(all_prompts.len(), 3, "There should be 3 prompts");

        // Perform a vector search
        let search_query = "first test prompt";
        let search_results = db.prompt_vector_search(search_query, 2).await?;
        assert_eq!(search_results.len(), 1, "There should be 1 search result");

        Ok(())
    }

    /// Helper function to create a CustomPrompt
    #[allow(dead_code)]
    fn create_custom_prompt(name: &str, prompt: &str) -> CustomPrompt {
        CustomPrompt {
            name: name.to_string(),
            prompt: prompt.to_string(),
            is_system: true,
            is_enabled: true,
            version: "1".to_string(),
            is_favorite: false,
            embedding: None,
        }
    }

    /// Not a real test, but can be used to generate the static prompts file
    #[allow(dead_code)]
    // #[tokio::test]
    async fn test_generate_static_prompts() {
        let generator = RemoteEmbeddingGenerator::new_default_local();

        let mut prompts_json_testing = Vec::new();
        let mut prompts_json = Vec::new();

        // Generate prompts for testing
        env::set_var("IS_TESTING", "1");
        let prompts_testing = vec![
            create_custom_prompt("Agility Story System", AGILITY_STORY_SYSTEM),
            create_custom_prompt("AI System", AI_SYSTEM),
            create_custom_prompt("Analyze Answers System", ANALYZE_ANSWERS_SYSTEM),
        ];
        println!("Number of testing prompts: {}", prompts_testing.len());

        for mut prompt in prompts_testing {
            let embedding = if let Some(embedding) = prompt.embedding {
                embedding
            } else {
                generator
                    .generate_embedding_default(&prompt.text_for_embedding())
                    .await
                    .unwrap()
                    .vector
            };

            prompt.embedding = Some(embedding);
            prompts_json_testing.push(json!(prompt));
        }

        // Generate prompts for production
        env::set_var("IS_TESTING", "0");
        let prompts = vec![
            create_custom_prompt("Agility Story System", AGILITY_STORY_SYSTEM),
            create_custom_prompt("AI System", AI_SYSTEM),
            create_custom_prompt("Analyze Answers System", ANALYZE_ANSWERS_SYSTEM),
            create_custom_prompt("Analyze Claims System", ANALYZE_CLAIMS_SYSTEM),
            create_custom_prompt("Analyze Debate System", ANALYZE_DEBATE_SYSTEM),
            create_custom_prompt("Analyze Incident System", ANALYZE_INCIDENT_SYSTEM),
            create_custom_prompt("Analyze Logs System", ANALYZE_LOGS_SYSTEM),
            create_custom_prompt("Analyze Malware System", ANALYZE_MALWARE_SYSTEM),
            create_custom_prompt("Analyze Paper System", ANALYZE_PAPER_SYSTEM),
            create_custom_prompt("Analyze Patent System", ANALYZE_PATENT_SYSTEM),
            create_custom_prompt("Analyze Personality System", ANALYZE_PERSONALITY_SYSTEM),
            create_custom_prompt("Analyze Presentation System", ANALYZE_PRESENTATION_SYSTEM),
            create_custom_prompt("Analyze Prose JSON System", ANALYZE_PROSE_JSON_SYSTEM),
            create_custom_prompt("Analyze Prose Pinker System", ANALYZE_PROSE_PINKER_SYSTEM),
            create_custom_prompt("Analyze Prose System", ANALYZE_PROSE_SYSTEM),
            create_custom_prompt("Analyze Spiritual Text System", ANALYZE_SPIRITUAL_TEXT_SYSTEM),
            create_custom_prompt("Analyze Tech Impact System", ANALYZE_TECH_IMPACT_SYSTEM),
            create_custom_prompt("Analyze Threat Report System", ANALYZE_THREAT_REPORT_SYSTEM),
            create_custom_prompt(
                "Analyze Threat Report Trends System",
                ANALYZE_THREAT_REPORT_TRENDS_SYSTEM,
            ),
            create_custom_prompt("Analyze Threat Report Trends User", ANALYZE_THREAT_REPORT_TRENDS_USER),
            create_custom_prompt("Analyze Threat Report User", ANALYZE_THREAT_REPORT_USER),
            create_custom_prompt("Answer Interview Question System", ANSWER_INTERVIEW_QUESTION_SYSTEM),
            create_custom_prompt(
                "Ask Secure By Design Questions System",
                ASK_SECURE_BY_DESIGN_QUESTIONS_SYSTEM,
            ),
            create_custom_prompt("Capture Thinkers Work System", CAPTURE_THINKERS_WORK_SYSTEM),
            create_custom_prompt("Check Agreement System", CHECK_AGREEMENT_SYSTEM),
            create_custom_prompt("Clean Text System", CLEAN_TEXT_SYSTEM),
            create_custom_prompt("Coding Master System", CODING_MASTER_SYSTEM),
            create_custom_prompt("Compare And Contrast System", COMPARE_AND_CONTRAST_SYSTEM),
            create_custom_prompt("Create 5 Sentence Summary System", CREATE_5_SENTENCE_SUMMARY_SYSTEM),
            create_custom_prompt("Create Academic Paper System", CREATE_ACADEMIC_PAPER_SYSTEM),
            create_custom_prompt("Create AI Jobs Analysis System", CREATE_AI_JOBS_ANALYSIS_SYSTEM),
            create_custom_prompt("Create Aphorisms System", CREATE_APHORISMS_SYSTEM),
            create_custom_prompt("Create Art Prompt System", CREATE_ART_PROMPT_SYSTEM),
            create_custom_prompt("Create Better Frame System", CREATE_BETTER_FRAME_SYSTEM),
            create_custom_prompt("Create Coding Project System", CREATE_CODING_PROJECT_SYSTEM),
            create_custom_prompt("Create Command System", CREATE_COMMAND_SYSTEM),
            create_custom_prompt("Create Cyber Summary System", CREATE_CYBER_SUMMARY_SYSTEM),
            create_custom_prompt("Create Git Diff Commit System", CREATE_GIT_DIFF_COMMIT_SYSTEM),
            create_custom_prompt("Create Graph From Input System", CREATE_GRAPH_FROM_INPUT_SYSTEM),
            create_custom_prompt("Create Hormozi Offer System", CREATE_HORMOZI_OFFER_SYSTEM),
            create_custom_prompt("Create Idea Compass System", CREATE_IDEA_COMPASS_SYSTEM),
            create_custom_prompt(
                "Create Investigation Visualization System",
                CREATE_INVESTIGATION_VISUALIZATION_SYSTEM,
            ),
            create_custom_prompt("Create Keynote System", CREATE_KEYNOTE_SYSTEM),
            create_custom_prompt("Create Logo System", CREATE_LOGO_SYSTEM),
            create_custom_prompt(
                "Create Markmap Visualization System",
                CREATE_MARKMAP_VISUALIZATION_SYSTEM,
            ),
            create_custom_prompt(
                "Create Mermaid Visualization System",
                CREATE_MERMAID_VISUALIZATION_SYSTEM,
            ),
            create_custom_prompt("Create Micro Summary System", CREATE_MICRO_SUMMARY_SYSTEM),
            create_custom_prompt(
                "Create Network Threat Landscape System",
                CREATE_NETWORK_THREAT_LANDSCAPE_SYSTEM,
            ),
            create_custom_prompt(
                "Create Network Threat Landscape User",
                CREATE_NETWORK_THREAT_LANDSCAPE_USER,
            ),
            create_custom_prompt("Create NPC System", CREATE_NPC_SYSTEM),
            create_custom_prompt("Create Pattern System", CREATE_PATTERN_SYSTEM),
            create_custom_prompt("Create Quiz System", CREATE_QUIZ_SYSTEM),
            create_custom_prompt("Create Reading Plan System", CREATE_READING_PLAN_SYSTEM),
            create_custom_prompt("Create Report Finding System", CREATE_REPORT_FINDING_SYSTEM),
            create_custom_prompt("Create Report Finding User", CREATE_REPORT_FINDING_USER),
            create_custom_prompt("Create Security Update System", CREATE_SECURITY_UPDATE_SYSTEM),
            create_custom_prompt("Create Show Intro System", CREATE_SHOW_INTRO_SYSTEM),
            create_custom_prompt("Create Sigma Rules System", CREATE_SIGMA_RULES_SYSTEM),
            create_custom_prompt("Create Stride Threat Model System", CREATE_STRIDE_THREAT_MODEL_SYSTEM),
            create_custom_prompt("Create Summary System", CREATE_SUMMARY_SYSTEM),
            create_custom_prompt("Create Tags System", CREATE_TAGS_SYSTEM),
            create_custom_prompt("Create Threat Scenarios System", CREATE_THREAT_SCENARIOS_SYSTEM),
            create_custom_prompt("Create Upgrade Pack System", CREATE_UPGRADE_PACK_SYSTEM),
            create_custom_prompt("Create Video Chapters System", CREATE_VIDEO_CHAPTERS_SYSTEM),
            create_custom_prompt("Create Visualization System", CREATE_VISUALIZATION_SYSTEM),
            create_custom_prompt("Explain Code System", EXPLAIN_CODE_SYSTEM),
            create_custom_prompt("Explain Code User", EXPLAIN_CODE_USER),
            create_custom_prompt("Explain Docs System", EXPLAIN_DOCS_SYSTEM),
            create_custom_prompt("Explain Project System", EXPLAIN_PROJECT_SYSTEM),
            create_custom_prompt("Explain Terms System", EXPLAIN_TERMS_SYSTEM),
            create_custom_prompt("Export Data As CSV System", EXPORT_DATA_AS_CSV_SYSTEM),
            create_custom_prompt(
                "Extract Algorithm Update Recommendations System",
                EXTRACT_ALGORITHM_UPDATE_RECOMMENDATIONS_SYSTEM,
            ),
            create_custom_prompt("Extract Article Wisdom System", EXTRACT_ARTICLE_WISDOM_SYSTEM),
            create_custom_prompt("Extract Article Wisdom User", EXTRACT_ARTICLE_WISDOM_USER),
            create_custom_prompt("Extract Book Ideas System", EXTRACT_BOOK_IDEAS_SYSTEM),
            create_custom_prompt(
                "Extract Book Recommendations System",
                EXTRACT_BOOK_RECOMMENDATIONS_SYSTEM,
            ),
            create_custom_prompt("Extract Business Ideas System", EXTRACT_BUSINESS_IDEAS_SYSTEM),
            create_custom_prompt("Extract Controversial Ideas System", EXTRACT_CONTROVERSIAL_IDEAS_SYSTEM),
            create_custom_prompt(
                "Extract Extraordinary Claims System",
                EXTRACT_EXTRAORDINARY_CLAIMS_SYSTEM,
            ),
            create_custom_prompt("Extract Ideas System", EXTRACT_IDEAS_SYSTEM),
            create_custom_prompt("Extract Insights System", EXTRACT_INSIGHTS_SYSTEM),
            create_custom_prompt("Extract Main Idea System", EXTRACT_MAIN_IDEA_SYSTEM),
            create_custom_prompt("Extract Patterns System", EXTRACT_PATTERNS_SYSTEM),
            create_custom_prompt("Extract POC System", EXTRACT_POC_SYSTEM),
            create_custom_prompt("Extract Predictions System", EXTRACT_PREDICTIONS_SYSTEM),
            create_custom_prompt("Extract Questions System", EXTRACT_QUESTIONS_SYSTEM),
            create_custom_prompt("Extract Recommendations System", EXTRACT_RECOMMENDATIONS_SYSTEM),
            create_custom_prompt("Extract References System", EXTRACT_REFERENCES_SYSTEM),
            create_custom_prompt("Extract Song Meaning System", EXTRACT_SONG_MEANING_SYSTEM),
            create_custom_prompt("Extract Sponsors System", EXTRACT_SPONSORS_SYSTEM),
            create_custom_prompt("Extract Videoid System", EXTRACT_VIDEOID_SYSTEM),
            create_custom_prompt("Extract Wisdom Agents System", EXTRACT_WISDOM_AGENTS_SYSTEM),
            create_custom_prompt("Extract Wisdom DM System", EXTRACT_WISDOM_DM_SYSTEM),
            create_custom_prompt("Extract Wisdom Nometa System", EXTRACT_WISDOM_NOMETA_SYSTEM),
            create_custom_prompt("Extract Wisdom System", EXTRACT_WISDOM_SYSTEM),
            create_custom_prompt("Find Hidden Message System", FIND_HIDDEN_MESSAGE_SYSTEM),
            create_custom_prompt("Find Logical Fallacies System", FIND_LOGICAL_FALLACIES_SYSTEM),
            create_custom_prompt("Generate Quiz System", GENERATE_QUIZ_SYSTEM),
            create_custom_prompt("Get Wow Per Minute System", GET_WOW_PER_MINUTE_SYSTEM),
            create_custom_prompt("Get YouTube RSS System", GET_YOUTUBE_RSS_SYSTEM),
            create_custom_prompt("Improve Academic Writing System", IMPROVE_ACADEMIC_WRITING_SYSTEM),
            create_custom_prompt("Improve Prompt System", IMPROVE_PROMPT_SYSTEM),
            create_custom_prompt("Improve Report Finding System", IMPROVE_REPORT_FINDING_SYSTEM),
            create_custom_prompt("Improve Report Finding User", IMPROVE_REPORT_FINDING_USER),
            create_custom_prompt("Improve Writing System", IMPROVE_WRITING_SYSTEM),
            create_custom_prompt("Label And Rate System", LABEL_AND_RATE_SYSTEM),
            create_custom_prompt("Official Pattern Template System", OFFICIAL_PATTERN_TEMPLATE_SYSTEM),
            create_custom_prompt("Provide Guidance System", PROVIDE_GUIDANCE_SYSTEM),
            create_custom_prompt("Rate AI Response System", RATE_AI_RESPONSE_SYSTEM),
            create_custom_prompt("Rate AI Result System", RATE_AI_RESULT_SYSTEM),
            create_custom_prompt("Rate Content System", RATE_CONTENT_SYSTEM),
            create_custom_prompt("Rate Content User", RATE_CONTENT_USER),
            create_custom_prompt("Rate Value System", RATE_VALUE_SYSTEM),
            create_custom_prompt("Raw Query System", RAW_QUERY_SYSTEM),
            create_custom_prompt("Recommend Artists System", RECOMMEND_ARTISTS_SYSTEM),
            create_custom_prompt("Show Fabric Options Markmap System", SHOW_FABRIC_OPTIONS_MARKMAP_SYSTEM),
            create_custom_prompt("Suggest Pattern System", SUGGEST_PATTERN_SYSTEM),
            create_custom_prompt("Suggest Pattern User", SUGGEST_PATTERN_USER),
            create_custom_prompt("Summarize Debate System", SUMMARIZE_DEBATE_SYSTEM),
            create_custom_prompt("Summarize Git Changes System", SUMMARIZE_GIT_CHANGES_SYSTEM),
            create_custom_prompt("Summarize Git Diff System", SUMMARIZE_GIT_DIFF_SYSTEM),
            create_custom_prompt("Summarize Lecture System", SUMMARIZE_LECTURE_SYSTEM),
            create_custom_prompt("Summarize Legislation System", SUMMARIZE_LEGISLATION_SYSTEM),
            create_custom_prompt("Summarize Micro System", SUMMARIZE_MICRO_SYSTEM),
            create_custom_prompt("Summarize Newsletter System", SUMMARIZE_NEWSLETTER_SYSTEM),
            create_custom_prompt("Summarize Paper System", SUMMARIZE_PAPER_SYSTEM),
            create_custom_prompt("Summarize Prompt System", SUMMARIZE_PROMPT_SYSTEM),
            create_custom_prompt("Summarize Pull Requests System", SUMMARIZE_PULL_REQUESTS_SYSTEM),
            create_custom_prompt("Summarize RPG Session System", SUMMARIZE_RPG_SESSION_SYSTEM),
            create_custom_prompt("Summarize System", SUMMARIZE_SYSTEM),
            create_custom_prompt("To Flashcards System", TO_FLASHCARDS_SYSTEM),
            create_custom_prompt("Tweet System", TWEET_SYSTEM),
            create_custom_prompt("Write Essay System", WRITE_ESSAY_SYSTEM),
            create_custom_prompt("Write Hackerone Report System", WRITE_HACKERONE_REPORT_SYSTEM),
            create_custom_prompt("Write Micro Essay System", WRITE_MICRO_ESSAY_SYSTEM),
            create_custom_prompt("Write Pull Request System", WRITE_PULL_REQUEST_SYSTEM),
            create_custom_prompt("Write Semgrep Rule System", WRITE_SEMGREP_RULE_SYSTEM),
        ];
        println!("Number of production prompts: {}", prompts.len());

        for mut prompt in prompts {
            let embedding = if let Some(embedding) = prompt.embedding {
                embedding
            } else {
                generator
                    .generate_embedding_default(&prompt.text_for_embedding())
                    .await
                    .unwrap()
                    .vector
            };

            prompt.embedding = Some(embedding);
            prompts_json.push(json!(prompt));
        }

        let json_data_testing =
            serde_json::to_string(&prompts_json_testing).expect("Failed to serialize testing prompts");
        let json_data = serde_json::to_string(&prompts_json).expect("Failed to serialize production prompts");

        // Print the current directory
        let current_dir = env::current_dir().expect("Failed to get current directory");
        println!("Current directory: {:?}", current_dir);

        let mut file = File::create("../../tmp/prompts_data.rs").expect("Failed to create file");
        writeln!(
            file,
            "pub static PROMPTS_JSON_TESTING: &str = r#\"{}\"#;",
            json_data_testing
        )
        .expect("Failed to write to file");
        writeln!(file, "pub static PROMPTS_JSON: &str = r#\"{}\"#;", json_data).expect("Failed to write to file");
    }
}
