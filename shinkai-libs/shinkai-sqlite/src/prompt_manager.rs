use crate::{SqliteManager, SqliteManagerError};
use bytemuck::cast_slice;
use rusqlite::{params, OptionalExtension, Result};
use serde_json::Value;
use shinkai_message_primitives::schemas::custom_prompt::CustomPrompt;

impl SqliteManager {
    pub async fn add_prompt(&self, prompt: &CustomPrompt) -> Result<CustomPrompt> {
        // Generate the embedding from the query string
        let embedding = self.generate_embeddings(&prompt.prompt).await?;
        self.add_prompt_with_vector(prompt, embedding)
    }

    // Adds a CustomPrompt entry to the shinkai_prompts table and its vector to prompt_vec_items
    pub fn add_prompt_with_vector(&self, prompt: &CustomPrompt, vector: Vec<f32>) -> Result<CustomPrompt> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO shinkai_prompts (
                name,
                is_system,
                is_enabled,
                version,
                prompt,
                is_favorite
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                prompt.name,
                prompt.is_system as i32,
                prompt.is_enabled as i32,
                prompt.version,
                prompt.prompt,
                prompt.is_favorite as i32,
            ],
        )?;

        let row_id = tx.last_insert_rowid();

        // Update the prompt's rowid
        let mut prompt = prompt.clone();
        prompt.rowid = Some(row_id);

        tx.execute(
            "INSERT INTO prompt_vec_items (rowid, embedding) VALUES (?1, ?2)",
            params![row_id, cast_slice(&vector)],
        )?;

        tx.commit()?;
        Ok(prompt)
    }

    // Retrieves CustomPrompt entries based on optional filters
    pub fn get_prompts(
        &self,
        name: Option<&str>,
        is_system: Option<bool>,
        is_enabled: Option<bool>,
    ) -> Result<Vec<CustomPrompt>> {
        let conn = self.get_connection()?;
        let mut query = "SELECT * FROM shinkai_prompts WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(n) = name {
            query.push_str(" AND name = ?");
            params.push(Box::new(n.to_string()));
        }
        if let Some(is_sys) = is_system {
            query.push_str(" AND is_system = ?");
            params.push(Box::new(is_sys as i32));
        }
        if let Some(is_en) = is_enabled {
            query.push_str(" AND is_enabled = ?");
            params.push(Box::new(is_en as i32));
        }

        let mut stmt = conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let prompt_iter = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(CustomPrompt {
                rowid: Some(row.get(0)?),
                name: row.get(1)?,
                is_system: row.get::<_, i32>(2)? != 0,
                is_enabled: row.get::<_, i32>(3)? != 0,
                version: row.get(4)?,
                prompt: row.get(5)?,
                is_favorite: row.get::<_, i32>(6)? != 0,
            })
        })?;

        prompt_iter.collect()
    }

    // Retrieves a single CustomPrompt by rowid
    pub fn get_prompt(&self, rowid: i64) -> Result<Option<CustomPrompt>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM shinkai_prompts WHERE rowid = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![rowid])?;

        if let Some(row) = rows.next()? {
            Ok(Some(CustomPrompt {
                rowid: Some(row.get(0)?),
                name: row.get(1)?,
                is_system: row.get::<_, i32>(2)? != 0,
                is_enabled: row.get::<_, i32>(3)? != 0,
                version: row.get(4)?,
                prompt: row.get(5)?,
                is_favorite: row.get::<_, i32>(6)? != 0,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn update_prompt(&self, prompt: &CustomPrompt) -> Result<()> {
        let embedding = self.generate_embeddings(&prompt.prompt).await?;
        self.update_prompt_with_vector(prompt, embedding)
    }

    // Updates or inserts a CustomPrompt and its vector
    pub fn update_prompt_with_vector(&self, prompt: &CustomPrompt, vector: Vec<f32>) -> Result<()> {
        // TODO: add error handling
        // if prompt.rowid.is_none() {
        //     return ;
        // }

        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        if let Some(_existing_prompt) = Self::get_prompt(self, prompt.rowid.unwrap())? {
            // Update the prompt details
            tx.execute(
                "UPDATE shinkai_prompts SET
                    name = ?1,
                    is_system = ?2,
                    is_enabled = ?3,
                    version = ?4,
                    prompt = ?5,
                    is_favorite = ?6
                WHERE rowid = ?7",
                params![
                    prompt.name,
                    prompt.is_system as i32,
                    prompt.is_enabled as i32,
                    prompt.version,
                    prompt.prompt,
                    prompt.is_favorite as i32,
                    prompt.rowid.unwrap(),
                ],
            )?;

            // Retrieve the rowid for the existing prompt
            let mut stmt = tx.prepare("SELECT rowid FROM shinkai_prompts WHERE name = ?1")?;
            let row_id: i64 = stmt.query_row(params![prompt.name], |row| row.get(0))?;

            // Update the embedding in the prompt_vec_items table
            tx.execute(
                "UPDATE prompt_vec_items SET embedding = ?1 WHERE rowid = ?2",
                params![cast_slice(&vector), row_id],
            )?;
        } else {
            // If the prompt does not exist, add it
            self.add_prompt_with_vector(prompt, vector)?;
        }

        tx.commit()?;
        Ok(())
    }

    // Retrieves all favorite prompts
    pub fn get_favorite_prompts(&self) -> Result<Vec<CustomPrompt>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM shinkai_prompts WHERE is_favorite = 1")?;
        let prompt_iter = stmt.query_map([], |row| {
            Ok(CustomPrompt {
                rowid: Some(row.get(0)?),
                name: row.get(1)?,
                is_system: row.get::<_, i32>(2)? != 0,
                is_enabled: row.get::<_, i32>(3)? != 0,
                version: row.get(4)?,
                prompt: row.get(5)?,
                is_favorite: row.get::<_, i32>(6)? != 0,
            })
        })?;

        prompt_iter.collect()
    }

    // Removes a prompt by name and its associated vector
    pub fn remove_prompt(&self, name: &str) -> Result<()> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Retrieve the rowid of the prompt to be deleted
        let rowid: Option<i64> = tx
            .query_row(
                "SELECT rowid FROM shinkai_prompts WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(rowid) = rowid {
            // Delete the prompt from shinkai_prompts
            tx.execute("DELETE FROM shinkai_prompts WHERE rowid = ?1", params![rowid])?;

            // Delete the associated vector from prompt_vec_items
            tx.execute("DELETE FROM prompt_vec_items WHERE rowid = ?1", params![rowid])?;
        }

        tx.commit()?;
        Ok(())
    }

    // Performs a vector search for prompts using a precomputed vector
    pub fn prompt_vector_search_with_vector(&self, vector: Vec<f32>, num_results: u64) -> Result<Vec<CustomPrompt>> {
        // Convert Vec<f32> to &[u8] using bytemuck
        let embedding_bytes: &[u8] = cast_slice(&vector);

        // Step 1: Perform the vector search to get rowids
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT rowid FROM prompt_vec_items 
             WHERE embedding MATCH ? 
             ORDER BY distance 
             LIMIT ?",
        )?;

        let rowids: Vec<i64> = stmt
            .query_map(params![embedding_bytes, num_results], |row| row.get(0))?
            .collect::<Result<Vec<i64>, _>>()?;

        // Step 2: Retrieve the corresponding CustomPrompt entries
        let mut prompts = Vec::new();
        for rowid in rowids {
            let mut stmt = conn.prepare("SELECT * FROM shinkai_prompts WHERE rowid = ?")?;
            let prompt = stmt.query_row(params![rowid], |row| {
                Ok(CustomPrompt {
                    rowid: Some(row.get(0)?),
                    name: row.get(1)?,
                    is_system: row.get::<_, i32>(2)? != 0,
                    is_enabled: row.get::<_, i32>(3)? != 0,
                    version: row.get(4)?,
                    prompt: row.get(5)?,
                    is_favorite: row.get::<_, i32>(6)? != 0,
                })
            })?;
            prompts.push(prompt);
        }

        Ok(prompts)
    }

    // Performs a vector search for prompts based on a query string
    pub async fn prompt_vector_search(&self, query: &str, num_results: u64) -> Result<Vec<CustomPrompt>> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // Generate the embedding from the query string
        let embedding = self.generate_embeddings(query).await?;

        // Use the new function to perform the search
        self.prompt_vector_search_with_vector(embedding, num_results)
    }

    // Retrieves the embedding of a prompt by rowid
    pub fn get_prompt_embedding_by_rowid(&self, rowid: i64) -> Result<Vec<f32>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT embedding FROM prompt_vec_items WHERE rowid = ?1")?;
        let embedding_bytes: Vec<u8> = stmt.query_row(params![rowid], |row| row.get(0))?;

        // Convert &[u8] back to Vec<f32> using bytemuck
        let embedding: &[f32] = bytemuck::cast_slice(&embedding_bytes);
        Ok(embedding.to_vec())
    }

    // Retrieves all CustomPrompt entries
    pub fn get_all_prompts(&self) -> Result<Vec<CustomPrompt>> {
        self.get_prompts(None, None, None)
    }

    /// Adds a list of prompts from a JSON value vector to the database
    pub fn add_prompts_from_json_values(&self, prompts: Vec<Value>) -> Result<()> {
        for prompt_value in prompts {
            // Extract fields from JSON
            let name = prompt_value["name"].as_str().unwrap().to_string();
            let is_system = prompt_value["is_system"].as_bool().unwrap();
            let is_enabled = prompt_value["is_enabled"].as_bool().unwrap();
            let version = prompt_value["version"].as_str().unwrap().to_string();
            let prompt_text = prompt_value["prompt"].as_str().unwrap().to_string();
            let is_favorite = prompt_value["is_favorite"].as_bool().unwrap();
            let embedding: Vec<f32> = prompt_value["embedding"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();

            // Create a CustomPrompt object
            let prompt = CustomPrompt {
                rowid: None,
                name,
                is_system,
                is_enabled,
                version,
                prompt: prompt_text,
                is_favorite,
            };

            // Add the prompt to the database
            self.add_prompt_with_vector(&prompt, embedding)?;
        }
        Ok(())
    }

    // Update the FTS table when inserting or updating a prompt
    pub async fn update_prompts_fts(&self, prompt: &CustomPrompt) -> Result<(), SqliteManagerError> {
        // Get a connection from the in-memory pool for FTS operations
        let mut fts_conn = self.fts_pool.get().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1), // Using a generic error code
                Some(e.to_string()),
            )
        })?;

        // Start a single transaction
        let tx = fts_conn.transaction()?;

        // Delete the existing entry
        tx.execute("DELETE FROM shinkai_prompts_fts WHERE name = ?1", params![prompt.name])?;

        // Insert the updated prompt name
        tx.execute(
            "INSERT INTO shinkai_prompts_fts(name) VALUES (?1)",
            params![prompt.name],
        )?;

        // Commit the transaction
        tx.commit()?;

        Ok(())
    }

    // Search the FTS table
    pub fn search_prompts_by_name(&self, query: &str) -> Result<Vec<CustomPrompt>, SqliteManagerError> {
        // Get a connection from the in-memory pool for FTS operations
        let fts_conn = self.fts_pool.get().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1), // Using a generic error code
                Some(e.to_string()),
            )
        })?;

        // Use the in-memory connection for FTS operations
        let mut stmt = fts_conn.prepare("SELECT name FROM shinkai_prompts_fts WHERE shinkai_prompts_fts MATCH ?1")?;

        let name_iter = stmt.query_map(params![query], |row| {
            let name: String = row.get(0)?;
            Ok(name)
        })?;

        let mut prompts = Vec::new();
        let conn = self.get_connection()?;

        for name_result in name_iter {
            let name = name_result.map_err(|e| {
                eprintln!("FTS query error: {}", e);
                SqliteManagerError::DatabaseError(e)
            })?;

            // Query the persistent database for the full prompt data
            let mut stmt = conn.prepare("SELECT * FROM shinkai_prompts WHERE name = ?1")?;
            let prompt = stmt.query_row(params![name], |row| {
                Ok(CustomPrompt {
                    rowid: Some(row.get(0)?),
                    name: row.get(1)?,
                    is_system: row.get::<_, i32>(2)? != 0,
                    is_enabled: row.get::<_, i32>(3)? != 0,
                    version: row.get(4)?,
                    prompt: row.get(5)?,
                    is_favorite: row.get::<_, i32>(6)? != 0,
                })
            })?;

            prompts.push(prompt);
        }

        Ok(prompts)
    }

    // Synchronize the FTS table with the main database
    pub async fn sync_prompts_fts_table(&self) -> Result<(), SqliteManagerError> {
        // Use the pooled connection to access the shinkai_prompts table
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT rowid, name FROM shinkai_prompts")?;
        let mut rows = stmt.query([])?;

        // Acquire a write lock on the fts_conn
        let fts_conn = self.fts_pool.get().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1), // Using a generic error code
                Some(e.to_string()),
            )
        })?;

        // Use the in-memory connection for FTS operations
        while let Some(row) = rows.next()? {
            let rowid: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            fts_conn.execute(
                "INSERT INTO shinkai_prompts_fts(rowid, name) VALUES (?1, ?2)
                 ON CONFLICT(rowid) DO UPDATE SET name = excluded.name",
                params![rowid, name],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::files::prompts_data::PROMPTS_JSON_TESTING;

    use super::*;
    use serde_json::Value;
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

    // Utility function to generate a vector of length 384 filled with a specified value
    fn generate_vector(value: f32) -> Vec<f32> {
        vec![value; 384]
    }

    #[tokio::test]
    async fn test_add_prompt_with_vector() {
        let manager = setup_test_db().await;
        let prompt = CustomPrompt {
            rowid: None,
            name: "Test Prompt".to_string(),
            is_system: false,
            is_enabled: true,
            version: "1.0".to_string(),
            prompt: "This is a test prompt.".to_string(),
            is_favorite: false,
        };

        let vector = generate_vector(0.1);
        let result = manager.add_prompt_with_vector(&prompt, vector);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_prompt_with_vector() {
        let manager = setup_test_db().await;
        let prompt = CustomPrompt {
            rowid: None,
            name: "Test Prompt".to_string(),
            is_system: false,
            is_enabled: true,
            version: "1.0".to_string(),
            prompt: "This is a test prompt.".to_string(),
            is_favorite: false,
        };

        let vector = generate_vector(0.1);
        let added_prompt = manager.add_prompt_with_vector(&prompt, vector).unwrap();

        // Test retrieval by rowid
        let retrieved_prompt_by_rowid = manager.get_prompt(added_prompt.rowid.unwrap()).unwrap();
        assert!(retrieved_prompt_by_rowid.is_some());
        assert_eq!(retrieved_prompt_by_rowid.unwrap().name, "Test Prompt");

        // Test retrieval by name using get_prompts
        let retrieved_prompts_by_name = manager.get_prompts(Some("Test Prompt"), None, None).unwrap();
        assert_eq!(retrieved_prompts_by_name.len(), 1);
        assert_eq!(retrieved_prompts_by_name[0].name, "Test Prompt");
    }

    #[tokio::test]
    async fn test_remove_prompt_with_vector() {
        let manager = setup_test_db().await;
        let prompt = CustomPrompt {
            rowid: None,
            name: "Test Prompt".to_string(),
            is_system: false,
            is_enabled: true,
            version: "1.0".to_string(),
            prompt: "This is a test prompt.".to_string(),
            is_favorite: false,
        };

        let vector = generate_vector(0.1);
        let added_prompt = manager.add_prompt_with_vector(&prompt, vector).unwrap();
        manager.remove_prompt("Test Prompt").unwrap();
        let retrieved_prompt = manager.get_prompt(added_prompt.rowid.unwrap()).unwrap();
        assert!(retrieved_prompt.is_none());
    }

    #[tokio::test]
    async fn test_list_prompts_with_vector() {
        let manager = setup_test_db().await;

        let prompt1 = CustomPrompt {
            rowid: None,
            name: "Prompt One".to_string(),
            is_system: false,
            is_enabled: true,
            version: "1.0".to_string(),
            prompt: "This is the first test prompt.".to_string(),
            is_favorite: false,
        };

        let prompt2 = CustomPrompt {
            rowid: None,
            name: "Prompt Two".to_string(),
            is_system: true,
            is_enabled: false,
            version: "1.1".to_string(),
            prompt: "This is the second test prompt.".to_string(),
            is_favorite: true,
        };

        let vector1 = generate_vector(0.1);
        let vector2 = generate_vector(0.2);

        manager.add_prompt_with_vector(&prompt1, vector1).unwrap();
        manager.add_prompt_with_vector(&prompt2, vector2).unwrap();

        let prompts = manager.get_prompts(None, None, None).unwrap();
        assert_eq!(prompts.len(), 2);
        assert!(prompts.iter().any(|p| p.name == "Prompt One"));
        assert!(prompts.iter().any(|p| p.name == "Prompt Two"));
    }

    #[tokio::test]
    async fn test_add_and_search_prompt_with_vector() {
        let manager = setup_test_db().await;

        // Create five CustomPrompts with different vectors
        let prompts = vec![
            ("Prompt 0.1", 0.1),
            ("Prompt 0.2", 0.2),
            ("Prompt 0.3", 0.3),
            ("Prompt 0.4", 0.4),
            ("Prompt 0.5", 0.5),
        ];

        for (name, value) in prompts {
            let prompt = CustomPrompt {
                rowid: None,
                name: name.to_string(),
                is_system: true,
                is_enabled: true,
                version: "1".to_string(),
                prompt: format!("This is a test prompt for {}.", name),
                is_favorite: false,
            };

            let vector = generate_vector(value);
            let result = manager.add_prompt_with_vector(&prompt, vector);
            assert!(result.is_ok());
        }

        // Perform a vector search using the specified search vector
        let search_vector = generate_vector(0.4);
        let search_results = manager.prompt_vector_search_with_vector(search_vector, 3).unwrap();

        // Check that the search results are not empty and that "Prompt 0.4" is the first result
        assert!(!search_results.is_empty());
        assert_eq!(search_results[0].name, "Prompt 0.4");

        // Check that the second result is either "Prompt 0.5" or "Prompt 0.3"
        assert!(search_results.len() > 1);
        assert!(search_results[1].name == "Prompt 0.5" || search_results[1].name == "Prompt 0.3");

        assert!(search_results.len() > 2);
        assert!(search_results[2].name == "Prompt 0.5" || search_results[2].name == "Prompt 0.3");
    }

    #[tokio::test]
    async fn test_update_prompt_and_embedding() {
        let manager = setup_test_db().await;

        // Add three prompts
        let prompts = vec![("Prompt 0.1", 0.1), ("Prompt 0.2", 0.2), ("Prompt 0.3", 0.3)];

        let mut rowid_to_update = None;
        for (name, value) in &prompts {
            let prompt = CustomPrompt {
                rowid: None,
                name: name.to_string(),
                is_system: false,
                is_enabled: true,
                version: "1.0".to_string(),
                prompt: format!("This is a test prompt for {}.", name),
                is_favorite: false,
            };

            let vector = generate_vector(*value);
            let added_prompt = manager.add_prompt_with_vector(&prompt, vector).unwrap();
            if name == &"Prompt 0.2" {
                rowid_to_update = added_prompt.rowid;
            }
        }

        // Update the second prompt to "Prompt 0.7" with vector 0.7
        let updated_prompt = CustomPrompt {
            rowid: rowid_to_update,
            name: "Prompt 0.7".to_string(),
            is_system: true,
            is_enabled: false,
            version: "1.1".to_string(),
            prompt: "This is an updated test prompt for Prompt 0.7.".to_string(),
            is_favorite: true,
        };

        let updated_vector = generate_vector(0.7);
        manager
            .update_prompt_with_vector(&updated_prompt, updated_vector)
            .unwrap();

        // Retrieve the updated prompt
        let retrieved_prompt = manager.get_prompt(rowid_to_update.unwrap()).unwrap();
        assert!(retrieved_prompt.is_some());
        let retrieved_prompt = retrieved_prompt.unwrap();
        assert_eq!(retrieved_prompt.name, "Prompt 0.7");
        assert_eq!(
            retrieved_prompt.prompt,
            "This is an updated test prompt for Prompt 0.7."
        );
        assert_eq!(retrieved_prompt.is_system, true);
        assert_eq!(retrieved_prompt.is_enabled, false);
        assert_eq!(retrieved_prompt.version, "1.1");
        assert_eq!(retrieved_prompt.is_favorite, true);

        // Retrieve the embedding for the updated prompt
        let retrieved_embedding = manager.get_prompt_embedding_by_rowid(rowid_to_update.unwrap()).unwrap();
        assert_eq!(retrieved_embedding, generate_vector(0.7));
    }

    #[tokio::test]
    async fn test_add_prompts_from_json() {
        let manager = setup_test_db().await;

        // Parse the JSON string into a Vec<Value>
        let prompts: Vec<Value> = serde_json::from_str(PROMPTS_JSON_TESTING).expect("Failed to parse JSON");

        // Measure the time taken to add prompts to the database
        let start_time = std::time::Instant::now();
        let result = manager.add_prompts_from_json_values(prompts.clone());
        let duration = start_time.elapsed();

        assert!(result.is_ok());

        // Verify that the number of prompts in the database matches the number in the JSON
        let all_prompts = manager.get_all_prompts().unwrap();
        assert_eq!(all_prompts.len(), prompts.len());

        // Print the duration
        println!("Time taken to add prompts from JSON: {:?}", duration);
    }

    #[tokio::test]
    async fn test_add_and_search_prompts_with_fts() {
        let manager = setup_test_db().await;

        // Add three prompts
        let prompts = vec![
            CustomPrompt {
                rowid: None,
                name: "Prompt Alpha".to_string(),
                is_system: false,
                is_enabled: true,
                version: "1.0".to_string(),
                prompt: "This is a test prompt for Alpha.".to_string(),
                is_favorite: false,
            },
            CustomPrompt {
                rowid: None,
                name: "Prompt Beta".to_string(),
                is_system: false,
                is_enabled: true,
                version: "1.0".to_string(),
                prompt: "This is a test prompt for Beta.".to_string(),
                is_favorite: false,
            },
            CustomPrompt {
                rowid: None,
                name: "Prompt Gamma".to_string(),
                is_system: false,
                is_enabled: true,
                version: "1.0".to_string(),
                prompt: "This is a test prompt for Gamma.".to_string(),
                is_favorite: false,
            },
        ];

        for prompt in &prompts {
            let vector = generate_vector(0.1);
            let result = manager.add_prompt_with_vector(prompt, vector);
            assert!(result.is_ok());

            // Update FTS table
            manager.update_prompts_fts(prompt).await.unwrap();
        }

        // Perform an FTS search for "Alpha"
        let search_results = manager.search_prompts_by_name("Alpha").unwrap();

        // Assert that the search results contain "Prompt Alpha"
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].name, "Prompt Alpha");

        // Perform an FTS search for "Beta"
        let search_results = manager.search_prompts_by_name("Beta").unwrap();

        // Assert that the search results contain "Prompt Beta"
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].name, "Prompt Beta");

        // Perform an FTS search for "Gamma"
        let search_results = manager.search_prompts_by_name("Gamma").unwrap();

        // Assert that the search results contain "Prompt Gamma"
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].name, "Prompt Gamma");
    }
}
