use serde::{Deserialize, Serialize};

/// The source of a Vector Resource as either the file contents of the source file itself,
/// or a pointer to the source file (either external such as URL, or a FileRef)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum VRSource {
    File(SourceFile),
    Pointer(SourcePointer),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SourceFile {
    // Define the fields for SourceFile
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SourceFilePointer {
    pub file_name: String,
    pub content_hash: String,
}

impl SourceFilePointer {
    pub fn shinkai_db_key(&self) -> String {
        format!("{}:{}", self.file_name, self.content_hash)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SourcePointer {
    FileRef(SourceFilePointer),
    ExternalURI(String),
}
