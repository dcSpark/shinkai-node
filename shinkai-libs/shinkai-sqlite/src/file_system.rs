use crate::{SqliteManager, SqliteManagerError};
use bytemuck::cast_slice;
use rusqlite::params;
use shinkai_message_primitives::{
    schemas::shinkai_fs::{ParsedFile, ShinkaiFileChunk},
    shinkai_utils::shinkai_path::ShinkaiPath,
};

impl SqliteManager {
    pub fn initialize_filesystem_tables(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        // parsed_files table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS parsed_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                relative_path TEXT NOT NULL UNIQUE,
                original_extension TEXT,
                description TEXT,
                source TEXT,
                embedding_model_used TEXT,
                keywords TEXT,
                distribution_info TEXT,
                created_time INTEGER,
                tags TEXT,
                total_tokens INTEGER,
                total_characters INTEGER
            );",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_parsed_files_rel_path ON parsed_files(relative_path);",
            [],
        )?;

        // chunks table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                parsed_file_id INTEGER NOT NULL REFERENCES parsed_files(id) ON DELETE CASCADE,
                position INTEGER NOT NULL,
                chunk TEXT NOT NULL,
                tokens INTEGER,
                characters INTEGER,
                metadata TEXT
            );",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_chunks_parsed_file_position ON chunks(parsed_file_id, position);",
            [],
        )?;

        // Create our new virtual table for chunk embeddings using sqlite-vec
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS chunk_vec USING vec0(
                embedding float[384],
                parsed_file_id INTEGER,
                +chunk_id INTEGER  -- Normal column recognized as chunk_id
            );",
            [],
        )?;

        Ok(())
    }

    // -------------------------
    // Parsed Files
    // -------------------------
    pub fn add_parsed_file(&self, pf: &ParsedFile) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM parsed_files WHERE relative_path = ?)",
            [&pf.relative_path],
            |row| row.get(0),
        )?;
        if exists {
            return Err(SqliteManagerError::DataAlreadyExists);
        }

        tx.execute(
            "INSERT INTO parsed_files (relative_path, original_extension, description, source, embedding_model_used, 
                                       keywords, distribution_info, created_time, tags, total_tokens, total_characters)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                pf.relative_path,
                pf.original_extension,
                pf.description,
                pf.source,
                pf.embedding_model_used,
                pf.keywords,
                pf.distribution_info,
                pf.created_time,
                pf.tags,
                pf.total_tokens,
                pf.total_characters
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_parsed_file_by_rel_path(&self, rel_path: &str) -> Result<Option<ParsedFile>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "
            SELECT id, relative_path, original_extension, description, source, embedding_model_used, keywords,
                   distribution_info, created_time, tags, total_tokens, total_characters
            FROM parsed_files
            WHERE relative_path = ?",
        )?;

        let res = stmt.query_row([rel_path], |row| {
            Ok(ParsedFile {
                id: row.get(0)?,
                relative_path: row.get(1)?,
                original_extension: row.get(2)?,
                description: row.get(3)?,
                source: row.get(4)?,
                embedding_model_used: row.get(5)?,
                keywords: row.get(6)?,
                distribution_info: row.get(7)?,
                created_time: row.get(8)?,
                tags: row.get(9)?,
                total_tokens: row.get(10)?,
                total_characters: row.get(11)?,
            })
        });

        match res {
            Ok(pf) => Ok(Some(pf)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SqliteManagerError::DatabaseError(e)),
        }
    }

    pub fn update_parsed_file(&self, pf: &ParsedFile) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM parsed_files WHERE id = ?)",
            [pf.id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        tx.execute(
            "UPDATE parsed_files
             SET relative_path = ?1, original_extension = ?2, description = ?3, source = ?4, embedding_model_used = ?5,
                 keywords = ?6, distribution_info = ?7, created_time = ?8, tags = ?9, total_tokens = ?10, total_characters = ?11
             WHERE id = ?12",
            params![
                pf.relative_path,
                pf.original_extension,
                pf.description,
                pf.source,
                pf.embedding_model_used,
                pf.keywords,
                pf.distribution_info,
                pf.created_time,
                pf.tags,
                pf.total_tokens,
                pf.total_characters,
                pf.id,
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn remove_parsed_file(&self, parsed_file_id: i64) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM parsed_files WHERE id = ?)",
            [parsed_file_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        tx.execute("DELETE FROM parsed_files WHERE id = ?", [parsed_file_id])?;
        tx.commit()?;

        Ok(())
    }

    // -------------------------
    // Chunk Embeddings
    // -------------------------

    /// Insert a new chunk (with text/metadata) into the `chunks` table
    /// and optionally insert the embedding into `chunk_vec` in one go.
    /// Returns the newly-created `chunk_id`.
    pub fn create_chunk_with_embedding(
        &self,
        chunk: &ShinkaiFileChunk,
        embedding: Option<&[f32]>,
    ) -> Result<i64, SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // 1) Verify the parsed file exists
        let parsed_file_exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM parsed_files WHERE id = ?)",
            [chunk.parsed_file_id],
            |row| row.get(0),
        )?;
        if !parsed_file_exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        // 2) Insert into `chunks` table
        tx.execute(
            "INSERT INTO chunks (parsed_file_id, position, chunk)
             VALUES (?1, ?2, ?3)",
            params![chunk.parsed_file_id, chunk.position, chunk.content],
        )?;

        // 3) Retrieve the auto-generated `chunk_id`
        let new_chunk_id = tx.last_insert_rowid();

        // 4) If we have an embedding, insert into `chunk_vec`
        if let Some(vec_data) = embedding {
            tx.execute(
                "INSERT INTO chunk_vec (embedding, parsed_file_id, chunk_id)
                 VALUES (?, ?, ?)",
                params![
                    bytemuck::cast_slice(vec_data), // from &[f32] to &[u8]
                    chunk.parsed_file_id,
                    new_chunk_id
                ],
            )?;
        }

        tx.commit()?;
        Ok(new_chunk_id)
    }

    /// Fetch a single chunk from `chunks` (text, metadata) plus
    /// *optionally* its embedding from `chunk_vec` in one query.
    /// Returns `None` if no chunk is found with that `chunk_id`.
    pub fn get_chunk_with_embedding(
        &self,
        chunk_id: i64,
    ) -> Result<Option<(ShinkaiFileChunk, Option<Vec<f32>>)>, SqliteManagerError> {
        // We'll do a LEFT JOIN: if there's an embedding in chunk_vec, we get it;
        // otherwise we get NULL and interpret that as "no embedding."
        let sql = r#"
            SELECT
                c.id AS c_id,
                c.parsed_file_id,
                c.position,
                c.chunk,
                cv.embedding AS vec_data
            FROM chunks c
            LEFT JOIN chunk_vec cv
                   ON c.id = cv.chunk_id
            WHERE c.id = ?
            LIMIT 1
        "#;

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(sql)?;

        let row_result = stmt.query_row([chunk_id], |row| {
            // Basic chunk columns:
            let c_id: i64 = row.get("c_id")?;
            let parsed_file_id: i64 = row.get("parsed_file_id")?;
            let position: i64 = row.get("position")?;
            let content: String = row.get("chunk")?;

            // Optional embedding column:
            let maybe_vec_data: Option<Vec<u8>> = row.get("vec_data")?;
            let embedding_opt: Option<Vec<f32>> = maybe_vec_data.map(|raw_bytes| {
                // Convert &[u8] back to Vec<f32>
                bytemuck::cast_slice(&raw_bytes).to_vec()
            });

            // Build the chunk
            let chunk_struct = ShinkaiFileChunk {
                chunk_id: Some(c_id),
                parsed_file_id,
                position,
                content,
            };

            Ok((chunk_struct, embedding_opt))
        });

        match row_result {
            Ok((chunk, embedding)) => Ok(Some((chunk, embedding))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SqliteManagerError::DatabaseError(e)),
        }
    }

    pub fn get_chunks_for_parsed_file(&self, parsed_file_id: i64) -> Result<Vec<ShinkaiFileChunk>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, parsed_file_id, position, chunk FROM chunks WHERE parsed_file_id = ? ORDER BY position",
        )?;
        let rows = stmt.query_map([parsed_file_id], |row| {
            Ok(ShinkaiFileChunk {
                chunk_id: Some(row.get(0)?),
                parsed_file_id: row.get(1)?,
                position: row.get(2)?,
                content: row.get(3)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Removes the chunk (and embedding if present) for the given `chunk_id` in a single transaction.
    pub fn remove_chunk_with_embedding(&self, chunk_id: i64) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // 1) Check that the chunk actually exists
        let chunk_exists: bool =
            tx.query_row("SELECT EXISTS(SELECT 1 FROM chunks WHERE id = ?)", [chunk_id], |row| {
                row.get(0)
            })?;
        if !chunk_exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        // 2) Remove embedding from `chunk_vec` (if any)
        tx.execute("DELETE FROM chunk_vec WHERE chunk_id = ?", [chunk_id])?;

        // 3) Remove the chunk itself
        tx.execute("DELETE FROM chunks WHERE id = ?", [chunk_id])?;

        tx.commit()?;
        Ok(())
    }

    // -------------------------
    // Folder Paths
    // -------------------------

    pub fn update_folder_paths(&self, old_prefix: &str, new_prefix: &str) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Construct a wildcard for the old_prefix
        let like_pattern = format!("{}%", old_prefix);

        tx.execute(
            "UPDATE parsed_files
             SET relative_path = REPLACE(relative_path, ?1, ?2)
             WHERE relative_path LIKE ?3",
            params![old_prefix, new_prefix, like_pattern],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn search_chunks(
        &self,
        parsed_file_id: i64,
        query_embedding: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<(i64, f64)>, SqliteManagerError> {
        let conn = self.get_connection()?;

        // Serialize the vector to a JSON array string
        let vector_json = serde_json::to_string(&query_embedding).map_err(|e| {
            eprintln!("Vector serialization error: {}", e);
            SqliteManagerError::SerializationError(e.to_string())
        })?;

        // SQL query to perform the vector search
        let sql = r#"
            SELECT v.chunk_id, v.distance
            FROM chunk_vec v
            WHERE v.embedding MATCH json(?)
            AND v.parsed_file_id = ?
            ORDER BY v.distance
            LIMIT ?
        "#;

        let mut stmt = conn.prepare(sql)?;

        // Execute the query and collect results using query_map
        let results: Vec<(i64, f64)> = stmt
            .query_map(params![vector_json, parsed_file_id, limit as i64], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
    }

    pub fn get_processed_files_in_directory(
        &self,
        directory_path: &str,
    ) -> Result<Vec<ParsedFile>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, relative_path, original_extension, description, source, embedding_model_used, keywords,
                    distribution_info, created_time, tags, total_tokens, total_characters
             FROM parsed_files
             WHERE relative_path LIKE ? AND relative_path NOT LIKE ?",
        )?;

        let like_pattern = format!("{}%", directory_path);
        let not_like_pattern = format!("{}%/%", directory_path);

        let rows = stmt.query_map(params![like_pattern, not_like_pattern], |row| {
            Ok(ParsedFile {
                id: row.get(0)?,
                relative_path: row.get(1)?,
                original_extension: row.get(2)?,
                description: row.get(3)?,
                source: row.get(4)?,
                embedding_model_used: row.get(5)?,
                keywords: row.get(6)?,
                distribution_info: row.get(7)?,
                created_time: row.get(8)?,
                tags: row.get(9)?,
                total_tokens: row.get(10)?,
                total_characters: row.get(11)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn get_parsed_file_by_shinkai_path(
        &self,
        path: &ShinkaiPath,
    ) -> Result<Option<ParsedFile>, SqliteManagerError> {
        let rel_path = path.relative_path();
        self.get_parsed_file_by_rel_path(rel_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    fn create_test_parsed_file(id: i64, relative_path: &str) -> ParsedFile {
        ParsedFile {
            id: Some(id),
            relative_path: relative_path.to_string(),
            original_extension: None,
            description: None,
            source: None,
            embedding_model_used: None,
            keywords: None,
            distribution_info: None,
            created_time: None,
            tags: None,
            total_tokens: None,
            total_characters: None,
        }
    }

    #[test]
    fn test_add_and_get_parsed_file() {
        let db = setup_test_db();

        let parsed_file = create_test_parsed_file(1, "file.txt");
        let result = db.add_parsed_file(&parsed_file);
        assert!(result.is_ok());

        let fetched = db.get_parsed_file_by_rel_path("file.txt").unwrap().unwrap();
        assert_eq!(fetched.relative_path, "file.txt");
    }

    #[test]
    fn test_add_duplicate_parsed_file() {
        let db = setup_test_db();

        let parsed_file1 = create_test_parsed_file(1, "file.txt");
        db.add_parsed_file(&parsed_file1).unwrap();

        let parsed_file2 = create_test_parsed_file(2, "file.txt");
        let result = db.add_parsed_file(&parsed_file2);
        assert!(matches!(result, Err(SqliteManagerError::DataAlreadyExists)));
    }

    #[test]
    fn test_update_parsed_file() {
        let db = setup_test_db();

        let mut parsed_file = create_test_parsed_file(1, "file.txt");
        db.add_parsed_file(&parsed_file).unwrap();

        // Update file path
        parsed_file.relative_path = "file_new.txt".to_string();
        let result = db.update_parsed_file(&parsed_file);
        assert!(result.is_ok());

        let fetched = db.get_parsed_file_by_rel_path("file_new.txt").unwrap().unwrap();
        assert_eq!(fetched.relative_path, "file_new.txt");
    }

    #[test]
    fn test_remove_parsed_file() {
        let db = setup_test_db();

        let parsed_file = create_test_parsed_file(1, "file.txt");
        db.add_parsed_file(&parsed_file).unwrap();

        let result = db.remove_parsed_file(1);
        assert!(result.is_ok());

        let fetched = db.get_parsed_file_by_rel_path("file.txt").unwrap();
        assert!(fetched.is_none());
    }

    #[test]
    fn test_remove_nonexistent_parsed_file() {
        let db = setup_test_db();

        let result = db.remove_parsed_file(999);
        assert!(matches!(result, Err(SqliteManagerError::DataNotFound)));
    }

    #[test]
    fn test_update_folder_paths() {
        let db = setup_test_db();

        let pf1 = create_test_parsed_file(1, "docs/reports/2024/january.txt");
        let pf2 = create_test_parsed_file(2, "docs/reports/2024/february.txt");
        let pf3 = create_test_parsed_file(3, "docs/reports/old_stuff/misc.txt");
        db.add_parsed_file(&pf1).unwrap();
        db.add_parsed_file(&pf2).unwrap();
        db.add_parsed_file(&pf3).unwrap();

        // Rename folder "docs/reports/2024/" to "docs/reports/2025/"
        db.update_folder_paths("docs/reports/2024/", "docs/reports/2025/")
            .unwrap();

        // Check updated files
        let updated_pf1 = db
            .get_parsed_file_by_rel_path("docs/reports/2025/january.txt")
            .unwrap()
            .unwrap();
        let updated_pf2 = db
            .get_parsed_file_by_rel_path("docs/reports/2025/february.txt")
            .unwrap()
            .unwrap();
        assert_eq!(updated_pf1.relative_path, "docs/reports/2025/january.txt");
        assert_eq!(updated_pf2.relative_path, "docs/reports/2025/february.txt");

        // Check that non-matching files are unaffected
        let unchanged_pf3 = db
            .get_parsed_file_by_rel_path("docs/reports/old_stuff/misc.txt")
            .unwrap()
            .unwrap();
        assert_eq!(unchanged_pf3.relative_path, "docs/reports/old_stuff/misc.txt");
    }

    #[test]
    fn test_get_files_in_directory() {
        let db = setup_test_db();

        // Add parsed files with different relative paths
        let pf1 = create_test_parsed_file(1, "docs/reports/2024/january.txt");
        let pf2 = create_test_parsed_file(2, "docs/reports/2024/february.txt");
        let pf3 = create_test_parsed_file(3, "docs/reports/2024/march/summary.txt");
        let pf4 = create_test_parsed_file(4, "docs/reports/old_stuff/misc.txt");
        db.add_parsed_file(&pf1).unwrap();
        db.add_parsed_file(&pf2).unwrap();
        db.add_parsed_file(&pf3).unwrap();
        db.add_parsed_file(&pf4).unwrap();

        // Retrieve files directly under "docs/reports/2024/"
        let files_in_directory = db.get_processed_files_in_directory("docs/reports/2024/").unwrap();

        // Check that only pf1 and pf2 are returned
        assert_eq!(files_in_directory.len(), 2);
        assert!(files_in_directory
            .iter()
            .any(|pf| pf.relative_path == "docs/reports/2024/january.txt"));
        assert!(files_in_directory
            .iter()
            .any(|pf| pf.relative_path == "docs/reports/2024/february.txt"));
        assert!(!files_in_directory
            .iter()
            .any(|pf| pf.relative_path == "docs/reports/2024/march/summary.txt"));
        assert!(!files_in_directory
            .iter()
            .any(|pf| pf.relative_path == "docs/reports/old_stuff/misc.txt"));
    }

    #[test]
    fn test_add_chunk_auto_id() {
        let db = setup_test_db();

        // Create and add a parsed file to associate with the chunk
        let parsed_file = create_test_parsed_file(1, "file.txt");
        db.add_parsed_file(&parsed_file).unwrap();

        // Create a chunk without specifying an id
        let chunk = ShinkaiFileChunk {
            chunk_id: None, // No id specified
            parsed_file_id: parsed_file.id.unwrap(),
            position: 1,
            content: "This is a test chunk.".to_string(),
        };

        // Add the chunk to the database
        let result = db.create_chunk_with_embedding(&chunk, None);
        assert!(result.is_ok());

        // Retrieve the chunk to verify it was added and has an auto-generated id
        let chunks = db.get_chunks_for_parsed_file(parsed_file.id.unwrap()).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].chunk_id.is_some()); // Check that the id is auto-generated
        assert_eq!(chunks[0].content, "This is a test chunk.");
    }

    #[test]
    fn test_vector_search_on_specific_parsed_file() {
        let db = setup_test_db();

        // Create and add two parsed files
        let parsed_file1 = create_test_parsed_file(1, "file1.txt");
        let parsed_file2 = create_test_parsed_file(2, "file2.txt");
        db.add_parsed_file(&parsed_file1).unwrap();
        db.add_parsed_file(&parsed_file2).unwrap();

        // Create and add chunks for the first parsed file
        let chunk1_file1 = ShinkaiFileChunk {
            chunk_id: None,
            parsed_file_id: parsed_file1.id.unwrap(),
            position: 1,
            content: "This is the first chunk of file1.".to_string(),
        };
        let chunk2_file1 = ShinkaiFileChunk {
            chunk_id: None,
            parsed_file_id: parsed_file1.id.unwrap(),
            position: 2,
            content: "This is the second chunk of file1.".to_string(),
        };
        db.create_chunk_with_embedding(&chunk1_file1, Some(&SqliteManager::generate_vector_for_testing(0.9))).unwrap();
        db.create_chunk_with_embedding(&chunk2_file1, Some(&SqliteManager::generate_vector_for_testing(0.9))).unwrap();

        // Create and add chunks for the second parsed file
        let chunk1_file2 = ShinkaiFileChunk {
            chunk_id: None,
            parsed_file_id: parsed_file2.id.unwrap(),
            position: 1,
            content: "This is the first chunk of file2.".to_string(),
        };
        let chunk2_file2 = ShinkaiFileChunk {
            chunk_id: None,
            parsed_file_id: parsed_file2.id.unwrap(),
            position: 2,
            content: "This is the second chunk of file2.".to_string(),
        };
        db.create_chunk_with_embedding(&chunk1_file2, Some(&SqliteManager::generate_vector_for_testing(0.9))).unwrap();
        db.create_chunk_with_embedding(&chunk2_file2, Some(&SqliteManager::generate_vector_for_testing(0.9))).unwrap();

        // Generate a mock query embedding
        let query_embedding = SqliteManager::generate_vector_for_testing(0.1);

        // Perform a vector search on the first parsed file
        let search_results = db
            .search_chunks(parsed_file1.id.unwrap(), query_embedding, 10)
            .unwrap();

        // Ensure that only chunks from the first parsed file are returned
        assert!(!search_results.is_empty());
        assert!(search_results.iter().all(|(chunk_id, _)| {
            db.get_chunks_for_parsed_file(parsed_file1.id.unwrap())
                .unwrap()
                .iter()
                .any(|chunk| chunk.chunk_id == Some(*chunk_id))
        }));

        // Ensure no chunks from the second parsed file are returned
        assert!(search_results.iter().all(|(chunk_id, _)| {
            !db.get_chunks_for_parsed_file(parsed_file2.id.unwrap())
                .unwrap()
                .iter()
                .any(|chunk| chunk.chunk_id == Some(*chunk_id))
        }));

        // Check that embeddings were added
        for (chunk_id, _) in search_results {
            let (chunk, embedding) = db.get_chunk_with_embedding(chunk_id).unwrap().unwrap();
            eprintln!("chunk: {:?}", chunk);
            assert!(embedding.is_some(), "Embedding should be present for chunk_id: {}", chunk_id);
        }
    }
}
