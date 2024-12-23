use serde::{Deserialize, Serialize};

/// Represents a file that has been parsed and indexed (e.g., split into chunks and possibly embedded).
/// This record stores metadata about the parsing process and the file itself, including its relative
/// path, extension, descriptions, and token/character counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedFile {
    /// Unique identifier for the parsed file entry.
    pub id: Option<i64>,
    /// The file's path relative to some base directory (e.g., "docs/manual.txt").
    pub relative_path: String,
    /// The original file extension (e.g., "txt", "md", "pdf").
    pub original_extension: Option<String>,
    /// A human-readable description of the file.
    pub description: Option<String>,
    /// The source of the file content (e.g., a URL or system component).
    pub source: Option<String>,
    /// The name or type of the embedding model used if embeddings were generated.
    pub embedding_model_used: Option<String>,
    /// Keywords or tags derived from or associated with the file.
    pub keywords: Option<String>,
    /// Information about how the file is distributed or shared.
    pub distribution_info: Option<String>,
    /// The timestamp when the file was parsed or created (in a UNIX timestamp format).
    pub created_time: Option<i64>,
    /// Arbitrary tags associated with the file for categorization or filtering.
    pub tags: Option<String>,
    /// The total number of tokens in the file (if known).
    pub total_tokens: Option<i64>,
    /// The total number of characters in the file (if known).
    pub total_characters: Option<i64>,
}

/// Represents a chunk of a processed file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShinkaiFileChunk {
    /// Unique identifier for the file chunk.
    pub chunk_id: Option<i64>,
    /// Identifier of the parsed file this chunk is associated with.
    pub parsed_file_id: i64,
    /// Sequence number of the chunk, indicating its order within the file.
    pub position: i64,
    /// The text content of this particular chunk.
    pub content: String,
}

/// Represents an embedding of a file chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShinkaiFileChunkEmbedding {
    /// Identifier of the file chunk this embedding is associated with.
    pub chunk_id: i64,
    /// Embedding vector for the file chunk.
    pub embedding: Vec<u8>, // Consider changing to Vec<f32> if desired
}

/// A struct that holds a collection of `ShinkaiFileChunk`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShinkaiFileChunkCollection {
    /// A set of chunks related to a parsed file.
    pub chunks: Vec<ShinkaiFileChunk>,
}

impl ShinkaiFileChunkCollection {
    /// Formats the data of all chunks into a single string that can be used as context.
    /// Respects a maximum character limit (`max_characters`).
    pub fn format_for_prompt(&self, max_characters: usize) -> Option<String> {
        let mut result = String::new();
        let mut remaining_chars = max_characters;

        for chunk in &self.chunks {
            let mut content = chunk.content.clone();
            if content.len() > remaining_chars {
                content = content.chars().take(remaining_chars).collect();
            }

            if content.is_empty() {
                break;
            }

            result.push_str(&content);
            result.push_str("\n\n");
            remaining_chars = remaining_chars.saturating_sub(content.len());
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}
