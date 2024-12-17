use crate::{SqliteManager, SqliteManagerError};
use rusqlite::{params, ToSql};
use shinkai_message_primitives::schemas::shinkai_fs::{ParsedFile, ShinkaiFileChunk, ShinkaiFileChunkEmbedding};

impl SqliteManager {
    pub fn initialize_filesystem_tables(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        // parsed_files table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS parsed_files (
                id INTEGER PRIMARY KEY,
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
                id INTEGER PRIMARY KEY,
                parsed_file_id INTEGER NOT NULL REFERENCES parsed_files(id) ON DELETE CASCADE,
                position INTEGER NOT NULL,
                chunk TEXT NOT NULL,
                tokens INTEGER,
                characters INTEGER,
                embedding BLOB,
                metadata TEXT
            );",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_chunks_parsed_file_position ON chunks(parsed_file_id, position);",
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
    // File Chunks
    // -------------------------
    pub fn add_chunk(&self, chunk: &ShinkaiFileChunk) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Ensure parsed_file_id exists
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM parsed_files WHERE id = ?)",
            [chunk.parsed_file_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        tx.execute(
            "INSERT INTO chunks (parsed_file_id, position, chunk)
             VALUES (?1, ?2, ?3)",
            params![chunk.parsed_file_id, chunk.position, chunk.content],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_chunks_for_parsed_file(&self, parsed_file_id: i64) -> Result<Vec<ShinkaiFileChunk>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, parsed_file_id, position, chunk FROM chunks WHERE parsed_file_id = ? ORDER BY position",
        )?;
        let rows = stmt.query_map([parsed_file_id], |row| {
            Ok(ShinkaiFileChunk {
                chunk_id: row.get(0)?,
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

    pub fn remove_chunks_for_parsed_file(&self, parsed_file_id: i64) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        tx.execute("DELETE FROM chunks WHERE parsed_file_id = ?", [parsed_file_id])?;

        tx.commit()?;
        Ok(())
    }

    // -------------------------
    // Chunk Embeddings
    // -------------------------
    pub fn add_chunk_embedding(&self, embedding: &ShinkaiFileChunkEmbedding) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Ensure chunk_id exists
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM chunks WHERE id = ?)",
            [embedding.chunk_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        tx.execute(
            "INSERT INTO file_chunk_embeddings (chunk_id, embedding)
             VALUES (?1, ?2)",
            params![embedding.chunk_id, &embedding.embedding as &dyn ToSql],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_chunk_embedding(&self, chunk_id: i64) -> Result<Option<ShinkaiFileChunkEmbedding>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT chunk_id, embedding FROM file_chunk_embeddings WHERE chunk_id = ?")?;
        let res = stmt.query_row([chunk_id], |row| {
            let embedding: Vec<u8> = row.get(1)?;
            Ok(ShinkaiFileChunkEmbedding {
                chunk_id: row.get(0)?,
                embedding,
            })
        });

        match res {
            Ok(e) => Ok(Some(e)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SqliteManagerError::DatabaseError(e)),
        }
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
            id,
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
}
