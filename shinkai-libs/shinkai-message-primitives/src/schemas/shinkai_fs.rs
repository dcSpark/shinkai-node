use serde::{Deserialize, Serialize};

/// Represents a directory in the Shinkai filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShinkaiDirectory {
    /// Unique identifier for the directory.
    pub dir_id: i64,
    /// Identifier of the parent directory, if any.
    pub parent_dir_id: Option<i64>,
    /// Name of the directory.
    pub name: String,
    /// Full path of the directory.
    pub full_path: String,
    /// Timestamp of when the directory was created.
    pub created_at: Option<i64>,
    /// Timestamp of when the directory was last modified.
    pub modified_at: Option<i64>,
}

/// Represents a file in the Shinkai filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShinkaiFile {
    /// Unique identifier for the file.
    pub file_id: i64,
    /// Identifier of the directory containing the file.
    pub directory_id: i64,
    /// Name of the file.
    pub name: String,
    /// Full path of the file.
    pub full_path: String,
    /// Size of the file in bytes.
    pub size: Option<i64>,
    /// Timestamp of when the file was created.
    pub created_at: Option<i64>,
    /// Timestamp of when the file was last modified.
    pub modified_at: Option<i64>,
    /// MIME type of the file.
    pub mime_type: Option<String>,
}

/// Represents a chunk of a file in the Shinkai filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShinkaiFileChunk {
    /// Unique identifier for the file chunk.
    pub chunk_id: i64,
    /// Identifier of the file to which this chunk belongs.
    pub file_id: i64,
    /// Sequence number of the chunk, indicating its order in the file.
    pub sequence: i64,
    /// Content of the file chunk.
    pub content: String,
}

/// Represents an embedding of a file chunk in the Shinkai filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShinkaiFileChunkEmbedding {
    /// Identifier of the file chunk this embedding is associated with.
    pub chunk_id: i64,
    /// Embedding vector for the file chunk.
    pub embedding: Vec<u8>, // TODO: change to f32
}

/// A struct that holds a collection of ShinkaiFileChunks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShinkaiFileChunkCollection {
    pub chunks: Vec<ShinkaiFileChunk>,
}

impl ShinkaiFileChunkCollection {
    /// Formats the data of all chunks into a single string that is ready
    /// to be included as part of a prompt to an LLM.
    /// Includes `max_characters` to allow specifying a hard-cap maximum that will be respected.
    pub fn format_for_prompt(&self, max_characters: usize) -> Option<String> {
        let mut result = String::new();
        let mut remaining_chars = max_characters;

        for chunk in &self.chunks {
            let mut content = chunk.content.clone();
            if content.len() > remaining_chars {
                content = content.chars().take(remaining_chars).collect();
            }

            if content.len() > remaining_chars {
                break;
            }

            result.push_str(&content);
            result.push_str("\n\n");
            remaining_chars -= content.len();
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}
