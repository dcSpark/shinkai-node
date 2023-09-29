use serde::{Deserialize, Serialize};
use std::fmt;

/// The source of a Vector Resource as either the file contents of the source file itself,
/// or a pointer to the source file (either external such as URL, or a FileRef)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum VRSource {
    File(SourceFile),
    Reference(SourceReference),
    None,
}

impl VRSource {
    /// Formats a printable string based on the source
    pub fn format_source_string(&self) -> String {
        match self {
            VRSource::File(file) => file.format_source_string(),
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

    /// Creates a VRSource reference using a SourceFile itself
    /// Do note, this will store the SourceFile
    pub fn new_file(file: SourceFile) -> Self {
        VRSource::File(file)
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
        format!("{}:{}", self.file_name, self.content_hash)
    }

    pub fn format_source_string(&self) -> String {
        format!("{}.{}", self.file_name, self.file_type)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SourceFileType {
    Pdf,
    Md,
    Txt,
    Epub,
    Doc,
    Docx,
    Rtf,  // Rich Text Format
    Odt,  // OpenDocument Text Document
    Html, // HTML Document
    Csv,  // Comma-Separated Values
    Xls,  // Excel Spreadsheet
    Xlsx, // Excel Open XML Spreadsheet
    Ppt,  // PowerPoint Presentation
    Pptx, // PowerPoint Open XML Presentation
}

impl fmt::Display for SourceFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                SourceFileType::Pdf => "pdf",
                SourceFileType::Md => "md",
                SourceFileType::Txt => "txt",
                SourceFileType::Epub => "epub",
                SourceFileType::Doc => "doc",
                SourceFileType::Docx => "docx",
                SourceFileType::Rtf => "rtf",
                SourceFileType::Odt => "odt",
                SourceFileType::Html => "html",
                SourceFileType::Csv => "csv",
                SourceFileType::Xls => "xls",
                SourceFileType::Xlsx => "xlsx",
                SourceFileType::Ppt => "ppt",
                SourceFileType::Pptx => "pptx",
            }
        )
    }
}
