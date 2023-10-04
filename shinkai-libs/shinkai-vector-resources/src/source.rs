use serde::{Deserialize, Serialize};
use std::fmt;

use crate::resource_errors::VectorResourceError;

/// The source of a Vector Resource as either the file contents of the source file itself,
/// or a pointer to the source file (either external such as URL, or a FileRef)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum VRSource {
    Reference(SourceReference),
    None,
}

impl VRSource {
    /// Formats a printable string based on the source
    pub fn format_source_string(&self) -> String {
        match self {
            VRSource::Reference(pointer) => pointer.format_source_string(),
            VRSource::None => String::from("None"),
        }
    }

    /// Creates a VRSource from an external URI or URL
    pub fn new_uri_ref(uri: &str) -> Self {
        VRSource::Reference(SourceReference::ExternalURI(uri.to_string()))
    }

    /// Creates a VRSource reference to an original source file
    pub fn new_source_file_ref(file_name: String, file_type: SourceFileType, content_hash: String) -> Self {
        VRSource::Reference(SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type,
            content_hash,
        }))
    }

    /// Creates a VRSource reference using an arbitrary String
    pub fn new_other_ref(other: String) -> Self {
        VRSource::Reference(SourceReference::Other(other))
    }

    /// Creates a VRSource which represents no/unknown source.
    pub fn none() -> Self {
        VRSource::None
    }

    /// Serializes the VRSource to a JSON string
    pub fn to_json(&self) -> Result<String, VectorResourceError> {
        serde_json::to_string(self).map_err(|_| VectorResourceError::FailedJSONParsing)
    }

    /// Deserializes a VRSource from a JSON string
    pub fn from_json(json: &str) -> Result<Self, VectorResourceError> {
        serde_json::from_str(json).map_err(|_| VectorResourceError::FailedJSONParsing)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
/// The source file that data was extracted from to create a VectorResource
pub struct SourceFile {
    pub file_name: String,
    pub file_type: SourceFileType,
    pub file_content: Vec<u8>,
}

impl SourceFile {
    /// Returns the size of the file content in bytes
    pub fn size(&self) -> usize {
        self.file_content.len()
    }

    /// Creates a new instance of SourceFile struct
    pub fn new(file_name: String, file_type: SourceFileType, file_content: Vec<u8>) -> Self {
        Self {
            file_name,
            file_type,
            file_content,
        }
    }

    pub fn format_source_string(&self) -> String {
        format!("{}.{}", self.file_name, self.file_type)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SourceReference {
    FileRef(SourceFileReference),
    ExternalURI(String),
    Other(String),
}

impl SourceReference {
    pub fn format_source_string(&self) -> String {
        match self {
            SourceReference::FileRef(pointer) => pointer.format_source_string(),
            SourceReference::ExternalURI(uri) => uri.clone(),
            SourceReference::Other(s) => s.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SourceFileReference {
    pub file_name: String,
    pub file_type: SourceFileType,
    pub content_hash: String,
}

impl SourceFileReference {
    /// The default key for this file in the Shinkai DB
    pub fn shinkai_db_key(&self) -> String {
        format!("{}:::{}", self.file_name, self.content_hash)
    }

    pub fn format_source_string(&self) -> String {
        format!("{}.{}", self.file_name, self.file_type)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SourceFileType {
    Document(SourceDocumentType),
    Image(SourceImageType),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SourceImageType {
    Png,
    Jpeg,
    Gif,
    Bmp,
    Tiff,
    Svg,
    Webp,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SourceDocumentType {
    Pdf,
    Md,
    Txt,
    Epub,
    Doc,
    Docx,
    Rtf,
    Odt,
    Html,
    Csv,
    Xls,
    Xlsx,
    Ppt,
    Pptx,
}

impl fmt::Display for SourceFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SourceFileType::Document(doc_type) => write!(f, "{}", doc_type),
            SourceFileType::Image(img_type) => write!(f, "{}", img_type),
        }
    }
}

impl fmt::Display for SourceImageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                SourceImageType::Png => "png",
                SourceImageType::Jpeg => "jpeg",
                SourceImageType::Gif => "gif",
                SourceImageType::Bmp => "bmp",
                SourceImageType::Tiff => "tiff",
                SourceImageType::Svg => "svg",
                SourceImageType::Webp => "webp",
            }
        )
    }
}

impl fmt::Display for SourceDocumentType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                SourceDocumentType::Pdf => "pdf",
                SourceDocumentType::Md => "md",
                SourceDocumentType::Txt => "txt",
                SourceDocumentType::Epub => "epub",
                SourceDocumentType::Doc => "doc",
                SourceDocumentType::Docx => "docx",
                SourceDocumentType::Rtf => "rtf",
                SourceDocumentType::Odt => "odt",
                SourceDocumentType::Html => "html",
                SourceDocumentType::Csv => "csv",
                SourceDocumentType::Xls => "xls",
                SourceDocumentType::Xlsx => "xlsx",
                SourceDocumentType::Ppt => "ppt",
                SourceDocumentType::Pptx => "pptx",
            }
        )
    }
}
