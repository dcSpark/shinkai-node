use crate::resource_errors::VRError;
use crate::unstructured::unstructured_parser::UnstructuredParser;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The source of a Vector Resource as either the file contents of the source file itself,
/// or a reference to the source file (either external such as URL, or a FileRef)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum VRSource {
    Reference(SourceReference),
    None,
}

impl VRSource {
    /// Formats a printable string based on the source
    pub fn format_source_string(&self) -> String {
        match self {
            VRSource::Reference(reference) => reference.format_source_string(),
            VRSource::None => String::from("None"),
        }
    }

    /// Creates a VRSource from an external URI or URL
    pub fn new_uri_ref(uri: &str) -> Self {
        Self::Reference(SourceReference::new_external_uri(uri.to_string()))
    }

    /// Creates a VRSource reference to an original source file
    pub fn new_source_file_ref(
        file_name: String,
        file_type: SourceFileType,
        content_hash: String,
        file_path: Option<String>,
    ) -> Self {
        VRSource::Reference(SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type,
            file_path,
            content_hash,
        }))
    }

    /// Creates a VRSource reference using an arbitrary String
    pub fn new_other_ref(other: &str) -> Self {
        Self::Reference(SourceReference::new_other(other.to_string()))
    }

    /// Creates a VRSource which represents no/unknown source.
    pub fn none() -> Self {
        VRSource::None
    }

    /// Serializes the VRSource to a JSON string
    pub fn to_json(&self) -> Result<String, VRError> {
        serde_json::to_string(self).map_err(|_| VRError::FailedJSONParsing)
    }

    /// Deserializes a VRSource from a JSON string
    pub fn from_json(json: &str) -> Result<Self, VRError> {
        serde_json::from_str(json).map_err(|_| VRError::FailedJSONParsing)
    }

    /// Creates a VRSource using file_name/content to auto-detect and create an instance of Self.
    /// Errors if can not detect matching extension in file_name.
    pub fn from_file(file_name: &str, file_buffer: &Vec<u8>) -> Result<Self, VRError> {
        let re = Regex::new(r"\.[^.]+$").unwrap();
        let file_name_without_extension = re.replace(file_name, "");
        let content_hash = UnstructuredParser::generate_data_hash(file_buffer);
        // Attempt to auto-detect, else use file extension
        let file_type = SourceFileType::detect_file_type(file_name)?;
        if file_name.starts_with("http") {
            Ok(VRSource::new_uri_ref(&file_name_without_extension))
        } else {
            let file_name_without_extension = file_name_without_extension.trim_start_matches("file://");
            Ok(VRSource::new_source_file_ref(
                file_name_without_extension.to_string(),
                file_type,
                content_hash,
                None,
            ))
        }
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
/// Type that acts as a reference to external file/content/data
pub enum SourceReference {
    FileRef(SourceFileReference),
    ExternalURI(String),
    Other(String),
}

impl SourceReference {
    pub fn format_source_string(&self) -> String {
        match self {
            SourceReference::FileRef(reference) => reference.format_source_string(),
            SourceReference::ExternalURI(uri) => uri.clone(),
            SourceReference::Other(s) => s.clone(),
        }
    }

    /// Creates a new SourceReference for a file, auto-detecting the file type
    /// by attempting to parse the extension in the file_name.
    /// Errors if extension is not found or not implemented yet.
    pub fn new_file_reference_auto_detect(
        file_name: String,
        content_hash: String,
        file_path: Option<String>,
    ) -> Result<Self, VRError> {
        let file_type = SourceFileType::detect_file_type(&file_name)?;
        Ok(SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type,
            content_hash,
            file_path,
        }))
    }

    /// Creates a new SourceReference for an image file
    pub fn new_file_image_reference(
        file_name: String,
        image_type: SourceImageType,
        content_hash: String,
        file_path: Option<String>,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Image(image_type),
            content_hash,
            file_path,
        })
    }

    /// Creates a new SourceReference for a document file
    pub fn new_file_doc_reference(
        file_name: String,
        doc_type: SourceDocumentType,
        content_hash: String,
        file_path: Option<String>,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Document(doc_type),
            content_hash,
            file_path,
        })
    }

    /// Creates a new SourceReference for an external URI
    pub fn new_external_uri(uri: String) -> Self {
        SourceReference::ExternalURI(uri)
    }

    /// Creates a new SourceReference for custom use cases
    pub fn new_other(s: String) -> Self {
        SourceReference::Other(s)
    }
}

impl fmt::Display for SourceReference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SourceReference::FileRef(reference) => write!(f, "{}", reference),
            SourceReference::ExternalURI(uri) => write!(f, "{}", uri),
            SourceReference::Other(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SourceFileReference {
    pub file_name: String,
    pub file_type: SourceFileType,
    pub file_path: Option<String>,
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

impl fmt::Display for SourceFileReference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "File Name: {}, File Type: {}, File Path: {}, Content Hash: {}",
            self.file_name,
            self.file_type,
            self.file_path.as_deref().unwrap_or("None"),
            self.content_hash
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SourceFileType {
    Document(SourceDocumentType),
    Image(SourceImageType),
}

impl SourceFileType {
    /// Given an input file_name with an extension,
    /// outputs the correct SourceFileType
    pub fn detect_file_type(file_name: &str) -> Result<Self, VRError> {
        let re = Regex::new(r"\.([a-zA-Z0-9]+)$").unwrap();
        let extension = re
            .captures(file_name)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str())
            .ok_or_else(|| VRError::CouldNotDetectFileType(file_name.to_string()))?;

        if let Ok(img_type) = SourceImageType::from_str(extension) {
            return Ok(SourceFileType::Image(img_type));
        }

        if let Ok(doc_type) = SourceDocumentType::from_str(extension) {
            return Ok(SourceFileType::Document(doc_type));
        }

        return Err(VRError::CouldNotDetectFileType(file_name.to_string()));
    }
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
    Ico,
    Heic,
    Raw,
    Other(String),
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
    Xml,
    Json,
    Yaml,
    Ps,
    Tex,
    Latex,
    Ods,
    Odp,
    Other(String),
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
                SourceImageType::Ico => "ico",
                SourceImageType::Heic => "heic",
                SourceImageType::Raw => "raw",
                SourceImageType::Other(s) => s.as_str(),
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
                SourceDocumentType::Xml => "xml",
                SourceDocumentType::Json => "json",
                SourceDocumentType::Yaml => "yaml",
                SourceDocumentType::Ps => "ps",
                SourceDocumentType::Tex => "tex",
                SourceDocumentType::Latex => "latex",
                SourceDocumentType::Ods => "ods",
                SourceDocumentType::Odp => "odp",
                SourceDocumentType::Other(s) => s.as_str(),
            }
        )
    }
}

impl FromStr for SourceImageType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "png" => Ok(SourceImageType::Png),
            "jpeg" => Ok(SourceImageType::Jpeg),
            "gif" => Ok(SourceImageType::Gif),
            "bmp" => Ok(SourceImageType::Bmp),
            "tiff" => Ok(SourceImageType::Tiff),
            "svg" => Ok(SourceImageType::Svg),
            "webp" => Ok(SourceImageType::Webp),
            "ico" => Ok(SourceImageType::Ico),
            "heic" => Ok(SourceImageType::Heic),
            "raw" => Ok(SourceImageType::Raw),
            _ => Err(()),
        }
    }
}

impl FromStr for SourceDocumentType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pdf" => Ok(SourceDocumentType::Pdf),
            "md" => Ok(SourceDocumentType::Md),
            "txt" => Ok(SourceDocumentType::Txt),
            "epub" => Ok(SourceDocumentType::Epub),
            "doc" => Ok(SourceDocumentType::Doc),
            "docx" => Ok(SourceDocumentType::Docx),
            "rtf" => Ok(SourceDocumentType::Rtf),
            "odt" => Ok(SourceDocumentType::Odt),
            "html" => Ok(SourceDocumentType::Html),
            "csv" => Ok(SourceDocumentType::Csv),
            "xls" => Ok(SourceDocumentType::Xls),
            "xlsx" => Ok(SourceDocumentType::Xlsx),
            "ppt" => Ok(SourceDocumentType::Ppt),
            "pptx" => Ok(SourceDocumentType::Pptx),
            "xml" => Ok(SourceDocumentType::Xml),
            "json" => Ok(SourceDocumentType::Json),
            "yaml" => Ok(SourceDocumentType::Yaml),
            "ps" => Ok(SourceDocumentType::Ps),
            "tex" => Ok(SourceDocumentType::Tex),
            "latex" => Ok(SourceDocumentType::Latex),
            "ods" => Ok(SourceDocumentType::Ods),
            "odp" => Ok(SourceDocumentType::Odp),
            _ => Err(()),
        }
    }
}
