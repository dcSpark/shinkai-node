use crate::{SqliteManager, SqliteManagerError};
use rusqlite::{params, ToSql};
use shinkai_message_primitives::schemas::shinkai_fs::{ShinkaiDirectory, ShinkaiFile, ShinkaiFileChunk, ShinkaiFileChunkEmbedding};

impl SqliteManager {
    pub fn initialize_filesystem_tables(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        // directories table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS directories (
                dir_id INTEGER PRIMARY KEY,
                parent_dir_id INTEGER REFERENCES directories(dir_id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                full_path TEXT NOT NULL UNIQUE,
                created_at INTEGER,
                modified_at INTEGER
            );",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_directories_parent_name ON directories(parent_dir_id, name);",
            [],
        )?;

        // files table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS files (
                file_id INTEGER PRIMARY KEY,
                directory_id INTEGER NOT NULL REFERENCES directories(dir_id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                full_path TEXT NOT NULL UNIQUE,
                size INTEGER,
                created_at INTEGER,
                modified_at INTEGER,
                mime_type TEXT
            );",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_dir_name ON files(directory_id, name);",
            [],
        )?;

        // parsed_files table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS parsed_files (
                id INTEGER PRIMARY KEY,
                file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
                name TEXT,
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
            "CREATE INDEX IF NOT EXISTS idx_parsed_files_file_id ON parsed_files(file_id);",
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
    // Directories
    // -------------------------
    pub fn add_directory(&self, dir: &ShinkaiDirectory) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Check if directory with the same full_path exists
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM directories WHERE full_path = ?)",
            [&dir.full_path],
            |row| row.get(0),
        )?;

        if exists {
            return Err(SqliteManagerError::DataAlreadyExists);
        }

        tx.execute(
            "INSERT INTO directories (parent_dir_id, name, full_path, created_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                dir.parent_dir_id,
                dir.name,
                dir.full_path,
                dir.created_at,
                dir.modified_at
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn remove_directory(&self, dir_id: i64) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Check if directory exists
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM directories WHERE dir_id = ?)",
            [dir_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        // Check if there are any files in this directory
        let file_count: i64 = tx.query_row("SELECT COUNT(*) FROM files WHERE directory_id = ?", [dir_id], |row| {
            row.get(0)
        })?;
        if file_count > 0 {
            return Err(SqliteManagerError::DirectoryNotEmpty);
        }

        // Check if there are any subdirectories
        let subdir_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM directories WHERE parent_dir_id = ?",
            [dir_id],
            |row| row.get(0),
        )?;
        if subdir_count > 0 {
            return Err(SqliteManagerError::DirectoryNotEmpty);
        }

        // If no files and no subdirectories, we can safely delete the directory
        tx.execute("DELETE FROM directories WHERE dir_id = ?", [dir_id])?;
        tx.commit()?;

        Ok(())
    }

    pub fn get_directory_by_id(&self, dir_id: i64) -> Result<Option<ShinkaiDirectory>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT dir_id, parent_dir_id, name, full_path, created_at, modified_at FROM directories WHERE dir_id = ?",
        )?;
        let res = stmt.query_row([dir_id], |row| {
            Ok(ShinkaiDirectory {
                dir_id: row.get(0)?,
                parent_dir_id: row.get(1)?,
                name: row.get(2)?,
                full_path: row.get(3)?,
                created_at: row.get(4)?,
                modified_at: row.get(5)?,
            })
        });

        match res {
            Ok(d) => Ok(Some(d)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SqliteManagerError::DatabaseError(e)),
        }
    }

    pub fn get_directory_by_path(&self, path: &str) -> Result<Option<ShinkaiDirectory>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT dir_id, parent_dir_id, name, full_path, created_at, modified_at FROM directories WHERE full_path = ?")?;
        let res = stmt.query_row([path], |row| {
            Ok(ShinkaiDirectory {
                dir_id: row.get(0)?,
                parent_dir_id: row.get(1)?,
                name: row.get(2)?,
                full_path: row.get(3)?,
                created_at: row.get(4)?,
                modified_at: row.get(5)?,
            })
        });

        match res {
            Ok(d) => Ok(Some(d)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SqliteManagerError::DatabaseError(e)),
        }
    }

    pub fn get_all_directories(&self) -> Result<Vec<ShinkaiDirectory>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT dir_id, parent_dir_id, name, full_path, created_at, modified_at FROM directories")?;
        let directories = stmt.query_map([], |row| {
            Ok(ShinkaiDirectory {
                dir_id: row.get(0)?,
                parent_dir_id: row.get(1)?,
                name: row.get(2)?,
                full_path: row.get(3)?,
                created_at: row.get(4)?,
                modified_at: row.get(5)?,
            })
        })?;

        let mut result = Vec::new();
        for d in directories {
            result.push(d?);
        }
        Ok(result)
    }

    pub fn update_directory(&self, dir: &ShinkaiDirectory) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM directories WHERE dir_id = ?)",
            [dir.dir_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        tx.execute(
            "UPDATE directories
             SET parent_dir_id = ?1, name = ?2, full_path = ?3, created_at = ?4, modified_at = ?5
             WHERE dir_id = ?6",
            params![
                dir.parent_dir_id,
                dir.name,
                dir.full_path,
                dir.created_at,
                dir.modified_at,
                dir.dir_id
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    // -------------------------
    // Files
    // -------------------------
    pub fn add_file(&self, file: &ShinkaiFile) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM files WHERE full_path = ?)",
            [&file.full_path],
            |row| row.get(0),
        )?;

        if exists {
            return Err(SqliteManagerError::DataAlreadyExists);
        }

        tx.execute(
            "INSERT INTO files (directory_id, name, full_path, size, created_at, modified_at, mime_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                file.directory_id,
                file.name,
                file.full_path,
                file.size,
                file.created_at,
                file.modified_at,
                file.mime_type
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn remove_file(&self, file_id: i64) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM files WHERE file_id = ?)",
            [file_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        tx.execute("DELETE FROM files WHERE file_id = ?", [file_id])?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_file_by_id(&self, file_id: i64) -> Result<Option<ShinkaiFile>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT file_id, directory_id, name, full_path, size, created_at, modified_at, mime_type FROM files WHERE file_id = ?")?;
        let res = stmt.query_row([file_id], |row| {
            Ok(ShinkaiFile {
                file_id: row.get(0)?,
                directory_id: row.get(1)?,
                name: row.get(2)?,
                full_path: row.get(3)?,
                size: row.get(4)?,
                created_at: row.get(5)?,
                modified_at: row.get(6)?,
                mime_type: row.get(7)?,
            })
        });

        match res {
            Ok(f) => Ok(Some(f)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SqliteManagerError::DatabaseError(e)),
        }
    }

    pub fn get_file_by_path(&self, path: &str) -> Result<Option<ShinkaiFile>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT file_id, directory_id, name, full_path, size, created_at, modified_at, mime_type FROM files WHERE full_path = ?")?;
        let res = stmt.query_row([path], |row| {
            Ok(ShinkaiFile {
                file_id: row.get(0)?,
                directory_id: row.get(1)?,
                name: row.get(2)?,
                full_path: row.get(3)?,
                size: row.get(4)?,
                created_at: row.get(5)?,
                modified_at: row.get(6)?,
                mime_type: row.get(7)?,
            })
        });

        match res {
            Ok(f) => Ok(Some(f)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SqliteManagerError::DatabaseError(e)),
        }
    }

    pub fn get_files_in_directory(&self, directory_id: i64) -> Result<Vec<ShinkaiFile>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT file_id, directory_id, name, full_path, size, created_at, modified_at, mime_type FROM files WHERE directory_id = ?")?;
        let files = stmt.query_map([directory_id], |row| {
            Ok(ShinkaiFile {
                file_id: row.get(0)?,
                directory_id: row.get(1)?,
                name: row.get(2)?,
                full_path: row.get(3)?,
                size: row.get(4)?,
                created_at: row.get(5)?,
                modified_at: row.get(6)?,
                mime_type: row.get(7)?,
            })
        })?;

        let mut result = Vec::new();
        for f in files {
            result.push(f?);
        }
        Ok(result)
    }

    pub fn update_file(&self, file: &ShinkaiFile) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM files WHERE file_id = ?)",
            [file.file_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        tx.execute(
            "UPDATE files
             SET directory_id = ?1, name = ?2, full_path = ?3, size = ?4, created_at = ?5, modified_at = ?6, mime_type = ?7
             WHERE file_id = ?8",
            params![
                file.directory_id,
                file.name,
                file.full_path,
                file.size,
                file.created_at,
                file.modified_at,
                file.mime_type,
                file.file_id
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    // -------------------------
    // File Chunks
    // -------------------------
    pub fn add_file_chunk(&self, chunk: &ShinkaiFileChunk) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // We do not enforce uniqueness on sequence here, but you could if needed.
        tx.execute(
            "INSERT INTO file_chunks (file_id, sequence, content)
             VALUES (?1, ?2, ?3)",
            params![chunk.file_id, chunk.sequence, chunk.content],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_chunks_for_file(&self, file_id: i64) -> Result<Vec<ShinkaiFileChunk>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT chunk_id, file_id, sequence, content FROM file_chunks WHERE file_id = ? ORDER BY sequence",
        )?;
        let chunks = stmt.query_map([file_id], |row| {
            Ok(ShinkaiFileChunk {
                chunk_id: row.get(0)?,
                file_id: row.get(1)?,
                sequence: row.get(2)?,
                content: row.get(3)?,
            })
        })?;

        let mut result = Vec::new();
        for c in chunks {
            result.push(c?);
        }
        Ok(result)
    }

    pub fn remove_chunks_for_file(&self, file_id: i64) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        tx.execute("DELETE FROM file_chunks WHERE file_id = ?", [file_id])?;

        tx.commit()?;
        Ok(())
    }

    // -------------------------
    // File Chunk Embeddings
    // -------------------------
    pub fn add_file_chunk_embedding(&self, embedding: &ShinkaiFileChunkEmbedding) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Ensure the chunk_id exists in file_chunks
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM file_chunks WHERE chunk_id = ?)",
            [embedding.chunk_id],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(SqliteManagerError::DataNotFound);
        }

        // If embeddings are one-to-one with file_chunks (chunk_id unique),
        // we can either INSERT or REPLACE. Here we assume no duplicates:
        tx.execute(
            "INSERT INTO file_chunk_embeddings (chunk_id, embedding)
             VALUES (?1, ?2)",
            params![embedding.chunk_id, &embedding.embedding as &dyn ToSql,],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_file_chunk_embedding(&self, chunk_id: i64) -> Result<Option<ShinkaiFileChunkEmbedding>, SqliteManagerError> {
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
        let model_type = EmbeddingModelType::OllamaTextEmbeddingsInference(
            OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M
        );

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    fn create_test_directory(
        dir_id: i64,
        parent_dir_id: Option<i64>,
        name: &str,
        full_path: &str
    ) -> ShinkaiDirectory {
        ShinkaiDirectory {
            dir_id,
            parent_dir_id,
            name: name.to_string(),
            full_path: full_path.to_string(),
            created_at: None,
            modified_at: None,
        }
    }

    fn create_test_file(
        file_id: i64,
        directory_id: i64,
        name: &str,
        full_path: &str
    ) -> ShinkaiFile {
        ShinkaiFile {
            file_id,
            directory_id,
            name: name.to_string(),
            full_path: full_path.to_string(),
            size: None,
            created_at: None,
            modified_at: None,
            mime_type: None,
        }
    }

    #[test]
    fn test_add_and_get_directory() {
        let db = setup_test_db();

        let root_dir = create_test_directory(1, None, "root", "/root");
        let result = db.add_directory(&root_dir);
        assert!(result.is_ok());

        // Get by ID
        let fetched = db.get_directory_by_id(1).unwrap().unwrap();
        assert_eq!(fetched.name, "root");
        assert_eq!(fetched.full_path, "/root");

        // Get by path
        let fetched_by_path = db.get_directory_by_path("/root").unwrap().unwrap();
        assert_eq!(fetched_by_path.dir_id, 1);
    }

    #[test]
    fn test_add_duplicate_directory() {
        let db = setup_test_db();

        let dir = create_test_directory(1, None, "root", "/root");
        db.add_directory(&dir).unwrap();

        // Trying to add the same full_path again should fail
        let dup_dir = create_test_directory(2, None, "root2", "/root");
        let result = db.add_directory(&dup_dir);
        assert!(matches!(result, Err(SqliteManagerError::DataAlreadyExists)));
    }

    #[test]
    fn test_get_all_directories() {
        let db = setup_test_db();

        let root_dir = create_test_directory(1, None, "root", "/root");
        let sub_dir = create_test_directory(2, Some(1), "sub", "/root/sub");

        db.add_directory(&root_dir).unwrap();
        db.add_directory(&sub_dir).unwrap();

        let all_dirs = db.get_all_directories().unwrap();
        assert_eq!(all_dirs.len(), 2);
        assert!(all_dirs.iter().any(|d| d.full_path == "/root"));
        assert!(all_dirs.iter().any(|d| d.full_path == "/root/sub"));
    }

    #[test]
    fn test_update_directory() {
        let db = setup_test_db();

        let mut dir = create_test_directory(1, None, "root", "/root");
        db.add_directory(&dir).unwrap();

        // Update directory name and path
        dir.name = "new_root".to_string();
        dir.full_path = "/new_root".to_string();
        let result = db.update_directory(&dir);
        assert!(result.is_ok());

        let fetched = db.get_directory_by_id(1).unwrap().unwrap();
        assert_eq!(fetched.name, "new_root");
        assert_eq!(fetched.full_path, "/new_root");
    }

    #[test]
    fn test_remove_directory_empty() {
        let db = setup_test_db();

        let dir = create_test_directory(1, None, "root", "/root");
        db.add_directory(&dir).unwrap();

        // Directory is empty, so removal should succeed
        let result = db.remove_directory(1);
        assert!(result.is_ok());

        let result = db.get_directory_by_id(1).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_directory_nonexistent() {
        let db = setup_test_db();
        // Try to remove a directory that doesn't exist
        let result = db.remove_directory(999);
        assert!(matches!(result, Err(SqliteManagerError::DataNotFound)));
    }

    #[test]
    fn test_remove_directory_with_files() {
        let db = setup_test_db();

        let root_dir = create_test_directory(1, None, "root", "/root");
        db.add_directory(&root_dir).unwrap();

        let file = create_test_file(1, 1, "file.txt", "/root/file.txt");
        db.add_file(&file).unwrap();

        // Attempt to remove a directory that contains a file
        let result = db.remove_directory(1);
        assert!(matches!(result, Err(SqliteManagerError::DirectoryNotEmpty)));
    }

    #[test]
    fn test_remove_directory_with_subdirectories() {
        let db = setup_test_db();

        let root_dir = create_test_directory(1, None, "root", "/root");
        db.add_directory(&root_dir).unwrap();

        let sub_dir = create_test_directory(2, Some(1), "sub", "/root/sub");
        db.add_directory(&sub_dir).unwrap();

        // Attempt to remove a directory that contains another directory
        let result = db.remove_directory(1);
        assert!(matches!(result, Err(SqliteManagerError::DirectoryNotEmpty)));
    }

    #[test]
    fn test_add_and_get_file() {
        let db = setup_test_db();

        let root_dir = create_test_directory(1, None, "root", "/root");
        db.add_directory(&root_dir).unwrap();

        let file = create_test_file(1, 1, "file.txt", "/root/file.txt");
        let result = db.add_file(&file);
        assert!(result.is_ok());

        let fetched = db.get_file_by_id(1).unwrap().unwrap();
        assert_eq!(fetched.name, "file.txt");
        assert_eq!(fetched.full_path, "/root/file.txt");

        let fetched_by_path = db.get_file_by_path("/root/file.txt").unwrap().unwrap();
        assert_eq!(fetched_by_path.file_id, 1);

        let files_in_dir = db.get_files_in_directory(1).unwrap();
        assert_eq!(files_in_dir.len(), 1);
        assert_eq!(files_in_dir[0].name, "file.txt");
    }

    #[test]
    fn test_add_duplicate_file() {
        let db = setup_test_db();

        let root_dir = create_test_directory(1, None, "root", "/root");
        db.add_directory(&root_dir).unwrap();

        let file1 = create_test_file(1, 1, "file.txt", "/root/file.txt");
        db.add_file(&file1).unwrap();

        let file2 = create_test_file(2, 1, "another_name", "/root/file.txt");
        let result = db.add_file(&file2);
        assert!(matches!(result, Err(SqliteManagerError::DataAlreadyExists)));
    }

    #[test]
    fn test_update_file() {
        let db = setup_test_db();

        let root_dir = create_test_directory(1, None, "root", "/root");
        db.add_directory(&root_dir).unwrap();

        let mut file = create_test_file(1, 1, "file.txt", "/root/file.txt");
        db.add_file(&file).unwrap();

        // Update file name and path
        file.name = "file_new.txt".to_string();
        file.full_path = "/root/file_new.txt".to_string();
        let result = db.update_file(&file);
        assert!(result.is_ok());

        let fetched = db.get_file_by_id(1).unwrap().unwrap();
        assert_eq!(fetched.name, "file_new.txt");
        assert_eq!(fetched.full_path, "/root/file_new.txt");
    }

    #[test]
    fn test_remove_file() {
        let db = setup_test_db();

        let root_dir = create_test_directory(1, None, "root", "/root");
        db.add_directory(&root_dir).unwrap();

        let file = create_test_file(1, 1, "file.txt", "/root/file.txt");
        db.add_file(&file).unwrap();

        let result = db.remove_file(1);
        assert!(result.is_ok());

        let fetched = db.get_file_by_id(1).unwrap();
        assert!(fetched.is_none());
    }

    #[test]
    fn test_remove_nonexistent_file() {
        let db = setup_test_db();

        let result = db.remove_file(999);
        assert!(matches!(result, Err(SqliteManagerError::DataNotFound)));
    }
}
