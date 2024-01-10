use crate::resource_errors::VRError;
use crate::unstructured::unstructured_parser::UnstructuredParser;
use crate::vector_resource::VRPath;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// What text chunking strategy was used to create this VR from the source file.
/// This is required for performing content validation/that it matches the VR nodes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TextChunkingStrategy {
    /// The default text chunking strategy implemented in VR lib using Unstructured.
    V1,
}

/// The source of a Vector Resource as either the file contents of the source file itself,
/// or a reference to the source file (either external such as URL, or a FileRef)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum VRSource {
    Reference(SourceReference),
    Notarized(NotarizedSourceReference),
    None,
}

impl VRSource {
    /// Formats a printable string based on the source
    pub fn format_source_string(&self) -> String {
        match self {
            VRSource::Reference(reference) => reference.format_source_string(),
            VRSource::Notarized(notarized_reference) => notarized_reference.format_source_string(),
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
        file_location: Option<String>,
    ) -> Self {
        VRSource::Reference(SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type,
            file_location,
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
        Ok(serde_json::to_string(self)?)
    }

    /// Deserializes a VRSource from a JSON string
    pub fn from_json(json: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(json)?)
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

/// Struct which holds the data of a source file which a VR was generated from
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SourceFile {
    Standard(StandardSourceFile),
    TLSNotarized(TLSNotarizedSourceFile),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
/// A standard source file that data was extracted from to create a VectorResource.
pub struct StandardSourceFile {
    pub file_name: String,
    pub file_type: SourceFileType,
    pub file_content: Vec<u8>,
    // Creation/publication time of the original content which is inside this struct
    pub original_creation_time: Datetime<Utc>,
}

impl StandardSourceFile {
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

    /// Serializes the SourceFile to a JSON string
    pub fn to_json(&self) -> Result<String, VRError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserializes a SourceFile from a JSON string
    pub fn from_json(json: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(json)?)
    }
}

/// Struct which holds the contents of the TLSNotary proof for the source file
struct TLSNotaryProof {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
/// The source file that data was extracted from to create a VectorResource
pub struct TLSNotarizedSourceFile {
    pub file_name: String,
    pub file_type: SourceFileType,
    pub file_content: Vec<u8>,
    // Creation/publication time of the original content which is inside this struct
    pub original_creation_time: Datetime<Utc>,
    pub proof: TLSNotaryProof,
}

impl TLSNotarizedSourceFile {
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

    /// Serializes the SourceFile to a JSON string
    pub fn to_json(&self) -> Result<String, VRError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserializes a SourceFile from a JSON string
    pub fn from_json(json: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(json)?)
    }
}

/// Type that acts as a reference to a notarized source file
/// (meaning one that has some cryptographic proof/signature of origin)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum NotarizedSourceReference {
    /// Reference to TLSNotary notarized web content
    TLSNotarized(TLSNotarizedReference),
}

impl NotarizedSourceReference {
    pub fn format_source_string(&self) -> String {
        match self {
            NotarizedSourceReference::TLSNotarized(reference) => reference.format_source_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TLSNotarizedReference {
    pub file_name: String,
    pub text_chunking_strategy: TextChunkingStrategy,
}

impl TLSNotarizedReference {
    pub fn format_source_string(&self) -> String {
        format!("{}.{}", self.file_name, self.file_type())
    }

    pub fn file_type(&self) -> SourceFileType {
        SourceFileType::Document(DocumentFileType::Html)
    }
}

impl fmt::Display for TLSNotarizedReference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "TLS Notarized File Name: {}, File Type: {}",
            self.file_name,
            self.file_type()
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
/// Type that acts as a reference to external file/content/data
pub enum SourceReference {
    /// A typed specific file
    FileRef(SourceFileReference),
    /// An arbitrary external URI
    ExternalURI(ExternalURIReference),
    Other(String),
}

/// Struct that represents an external URI like a website URL which
/// has not been downloaded into a SourceFile, but is just referenced.
pub struct ExternalURIReference {
    pub uri: String,
    // Creation/publication time of the original content which is specified at the uri
    pub original_creation_time: Datetime<Utc>,
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
        file_location: Option<String>,
    ) -> Result<Self, VRError> {
        let file_type = SourceFileType::detect_file_type(&file_name)?;
        Ok(SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type,
            content_hash,
            file_location,
        }))
    }

    /// Creates a new SourceReference for an image file
    pub fn new_file_image_reference(
        file_name: String,
        image_type: ImageFileType,
        content_hash: String,
        file_location: Option<String>,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Image(image_type),
            content_hash,
            file_location,
        })
    }

    /// Creates a new SourceReference for a document file
    pub fn new_file_doc_reference(
        file_name: String,
        doc_type: DocumentFileType,
        content_hash: String,
        file_location: Option<String>,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Document(doc_type),
            content_hash,
            file_location,
        })
    }

    /// Creates a new SourceReference for a code file
    pub fn new_file_code_reference(
        file_name: String,
        code_type: CodeFileType,
        content_hash: String,
        file_location: Option<String>,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Code(code_type),
            content_hash,
            file_location,
        })
    }

    /// Creates a new SourceReference for a config file
    pub fn new_file_config_reference(
        file_name: String,
        config_type: ConfigFileType,
        content_hash: String,
        file_location: Option<String>,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::ConfigFileType(config_type),
            content_hash,
            file_location,
        })
    }

    /// Creates a new SourceReference for a video file
    pub fn new_file_video_reference(
        file_name: String,
        video_type: VideoFileType,
        content_hash: String,
        file_location: Option<String>,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Video(video_type),
            content_hash,
            file_location,
        })
    }

    /// Creates a new SourceReference for an audio file
    pub fn new_file_audio_reference(
        file_name: String,
        audio_type: AudioFileType,
        content_hash: String,
        file_location: Option<String>,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Audio(audio_type),
            content_hash,
            file_location,
        })
    }

    /// Creates a new SourceReference for a Shinkai file
    pub fn new_file_shinkai_reference(
        file_name: String,
        shinkai_type: ShinkaiFileType,
        content_hash: String,
        file_location: Option<String>,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Shinkai(shinkai_type),
            content_hash,
            file_location,
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
    /// Local path or external URI to file TODO: likely remove this as is tracked by VecFS
    pub file_location: Option<String>,
    pub content_hash: String,
    pub text_chunking_strategy: TextChunkingStrategy,
}

impl SourceFileReference {
    pub fn format_source_string(&self) -> String {
        format!("{}.{}", self.file_name, self.file_type)
    }
}

impl fmt::Display for SourceFileReference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "File Name: {}, File Type: {}, File Location: {}, Content Hash: {}",
            self.file_name,
            self.file_type,
            self.file_location.as_deref().unwrap_or("None"),
            self.content_hash
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SourceFileType {
    Document(DocumentFileType),
    Image(ImageFileType),
    Code(CodeFileType),
    ConfigFileType(ConfigFileType),
    Video(VideoFileType),
    Audio(AudioFileType),
    Shinkai(ShinkaiFileType),
}

impl SourceFileType {
    /// Given an input file_name with an extension, outputs the correct SourceFileType
    /// or an error if the extension cannot be found or is not supported yet
    pub fn detect_file_type(file_name: &str) -> Result<Self, VRError> {
        let re = Regex::new(r"\.([a-zA-Z0-9]+)$").unwrap();
        let extension = if file_name.starts_with('.') {
            file_name
        } else {
            re.captures(file_name)
                .and_then(|cap| cap.get(1))
                .map(|m| m.as_str())
                .ok_or_else(|| VRError::CouldNotDetectFileType(file_name.to_string()))?
        };

        if let Ok(img_type) = ImageFileType::from_str(extension) {
            return Ok(SourceFileType::Image(img_type));
        }

        if let Ok(doc_type) = DocumentFileType::from_str(extension) {
            return Ok(SourceFileType::Document(doc_type));
        }

        return Err(VRError::CouldNotDetectFileType(file_name.to_string()));
    }

    /// Clones and cleans the input string of its file extension at the end, if it exists.
    pub fn clean_string_of_extension(file_name: &str) -> String {
        let re = Regex::new(r"\.[^.]+$").unwrap();
        let file_name_without_extension = re.replace(file_name, "");
        file_name_without_extension.to_string()
    }
}

impl fmt::Display for SourceFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SourceFileType::Document(doc_type) => write!(f, "{}", doc_type),
            SourceFileType::Image(img_type) => write!(f, "{}", img_type),
            SourceFileType::Code(code_type) => write!(f, "{}", code_type),
            SourceFileType::ConfigFileType(config_type) => write!(f, "{}", config_type),
            SourceFileType::Video(video_type) => write!(f, "{}", video_type),
            SourceFileType::Audio(audio_type) => write!(f, "{}", audio_type),
            SourceFileType::Shinkai(shinkai_type) => write!(f, "{}", shinkai_type),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ImageFileType {
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

impl fmt::Display for ImageFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ImageFileType::Png => "png",
                ImageFileType::Jpeg => "jpeg",
                ImageFileType::Gif => "gif",
                ImageFileType::Bmp => "bmp",
                ImageFileType::Tiff => "tiff",
                ImageFileType::Svg => "svg",
                ImageFileType::Webp => "webp",
                ImageFileType::Ico => "ico",
                ImageFileType::Heic => "heic",
                ImageFileType::Raw => "raw",
                ImageFileType::Other(s) => s.as_str(),
            }
        )
    }
}

impl FromStr for ImageFileType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "png" => Ok(ImageFileType::Png),
            "jpeg" => Ok(ImageFileType::Jpeg),
            "gif" => Ok(ImageFileType::Gif),
            "bmp" => Ok(ImageFileType::Bmp),
            "tiff" => Ok(ImageFileType::Tiff),
            "svg" => Ok(ImageFileType::Svg),
            "webp" => Ok(ImageFileType::Webp),
            "ico" => Ok(ImageFileType::Ico),
            "heic" => Ok(ImageFileType::Heic),
            "raw" => Ok(ImageFileType::Raw),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum DocumentFileType {
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

impl fmt::Display for DocumentFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DocumentFileType::Pdf => "pdf",
                DocumentFileType::Md => "md",
                DocumentFileType::Txt => "txt",
                DocumentFileType::Epub => "epub",
                DocumentFileType::Doc => "doc",
                DocumentFileType::Docx => "docx",
                DocumentFileType::Rtf => "rtf",
                DocumentFileType::Odt => "odt",
                DocumentFileType::Html => "html",
                DocumentFileType::Csv => "csv",
                DocumentFileType::Xls => "xls",
                DocumentFileType::Xlsx => "xlsx",
                DocumentFileType::Ppt => "ppt",
                DocumentFileType::Pptx => "pptx",
                DocumentFileType::Xml => "xml",
                DocumentFileType::Json => "json",
                DocumentFileType::Yaml => "yaml",
                DocumentFileType::Ps => "ps",
                DocumentFileType::Tex => "tex",
                DocumentFileType::Latex => "latex",
                DocumentFileType::Ods => "ods",
                DocumentFileType::Odp => "odp",
                DocumentFileType::Other(s) => s.as_str(),
            }
        )
    }
}

impl FromStr for DocumentFileType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pdf" => Ok(DocumentFileType::Pdf),
            "md" => Ok(DocumentFileType::Md),
            "txt" => Ok(DocumentFileType::Txt),
            "epub" => Ok(DocumentFileType::Epub),
            "doc" => Ok(DocumentFileType::Doc),
            "docx" => Ok(DocumentFileType::Docx),
            "rtf" => Ok(DocumentFileType::Rtf),
            "odt" => Ok(DocumentFileType::Odt),
            "html" => Ok(DocumentFileType::Html),
            "csv" => Ok(DocumentFileType::Csv),
            "xls" => Ok(DocumentFileType::Xls),
            "xlsx" => Ok(DocumentFileType::Xlsx),
            "ppt" => Ok(DocumentFileType::Ppt),
            "pptx" => Ok(DocumentFileType::Pptx),
            "xml" => Ok(DocumentFileType::Xml),
            "json" => Ok(DocumentFileType::Json),
            "yaml" => Ok(DocumentFileType::Yaml),
            "ps" => Ok(DocumentFileType::Ps),
            "tex" => Ok(DocumentFileType::Tex),
            "latex" => Ok(DocumentFileType::Latex),
            "ods" => Ok(DocumentFileType::Ods),
            "odp" => Ok(DocumentFileType::Odp),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum CodeFileType {
    Python,
    Java,
    JavaScript,
    TypeScript,
    C,
    Cpp,
    CppHeader,
    CSharp,
    Go,
    Rust,
    Swift,
    Kotlin,
    Php,
    Ruby,
    Other(String),
}

impl fmt::Display for CodeFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                CodeFileType::Python => "py",
                CodeFileType::Java => "java",
                CodeFileType::JavaScript => "js",
                CodeFileType::TypeScript => "ts",
                CodeFileType::C => "c",
                CodeFileType::Cpp => "cpp",
                CodeFileType::CppHeader => "h",
                CodeFileType::CSharp => "cs",
                CodeFileType::Go => "go",
                CodeFileType::Rust => "rs",
                CodeFileType::Swift => "swift",
                CodeFileType::Kotlin => "kt",
                CodeFileType::Php => "php",
                CodeFileType::Ruby => "rb",
                CodeFileType::Other(s) => s.as_str(),
            }
        )
    }
}

impl FromStr for CodeFileType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "py" => Ok(CodeFileType::Python),
            "java" => Ok(CodeFileType::Java),
            "js" => Ok(CodeFileType::JavaScript),
            "ts" => Ok(CodeFileType::TypeScript),
            "c" => Ok(CodeFileType::C),
            "cpp" => Ok(CodeFileType::Cpp),
            "h" => Ok(CodeFileType::CppHeader),
            "cs" => Ok(CodeFileType::CSharp),
            "go" => Ok(CodeFileType::Go),
            "rs" => Ok(CodeFileType::Rust),
            "swift" => Ok(CodeFileType::Swift),
            "kt" => Ok(CodeFileType::Kotlin),
            "php" => Ok(CodeFileType::Php),
            "rb" => Ok(CodeFileType::Ruby),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ConfigFileType {
    Toml,
    Ini,
    Eslint,
    Prettier,
    Webpack,
    Dockerfile,
    Gitignore,
    Other(String),
}

impl fmt::Display for ConfigFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ConfigFileType::Toml => "toml",
                ConfigFileType::Ini => "ini",
                ConfigFileType::Eslint => ".eslintrc",
                ConfigFileType::Prettier => ".prettierrc",
                ConfigFileType::Webpack => "webpack.config.js",
                ConfigFileType::Dockerfile => "Dockerfile",
                ConfigFileType::Gitignore => ".gitignore",
                ConfigFileType::Other(s) => s.as_str(),
            }
        )
    }
}

impl FromStr for ConfigFileType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "toml" => Ok(ConfigFileType::Toml),
            "ini" => Ok(ConfigFileType::Ini),
            ".eslintrc" => Ok(ConfigFileType::Eslint),
            ".prettierrc" => Ok(ConfigFileType::Prettier),
            "webpack.config.js" => Ok(ConfigFileType::Webpack),
            "Dockerfile" => Ok(ConfigFileType::Dockerfile),
            ".gitignore" => Ok(ConfigFileType::Gitignore),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AudioFileType {
    Mp3,
    Wav,
    Ogg,
    Flac,
    Aac,
    Wma,
    Alac,
    Ape,
    Dsf,
    M4a,
    Opus,
    Ra,
    Au,
    Aiff,
    Other(String),
}

impl fmt::Display for AudioFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AudioFileType::Mp3 => "mp3",
                AudioFileType::Wav => "wav",
                AudioFileType::Ogg => "ogg",
                AudioFileType::Flac => "flac",
                AudioFileType::Aac => "aac",
                AudioFileType::Wma => "wma",
                AudioFileType::Alac => "alac",
                AudioFileType::Ape => "ape",
                AudioFileType::Dsf => "dsf",
                AudioFileType::M4a => "m4a",
                AudioFileType::Opus => "opus",
                AudioFileType::Ra => "ra",
                AudioFileType::Au => "au",
                AudioFileType::Aiff => "aiff",
                AudioFileType::Other(s) => s.as_str(),
            }
        )
    }
}

impl FromStr for AudioFileType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mp3" => Ok(AudioFileType::Mp3),
            "wav" => Ok(AudioFileType::Wav),
            "ogg" => Ok(AudioFileType::Ogg),
            "flac" => Ok(AudioFileType::Flac),
            "aac" => Ok(AudioFileType::Aac),
            "wma" => Ok(AudioFileType::Wma),
            "alac" => Ok(AudioFileType::Alac),
            "ape" => Ok(AudioFileType::Ape),
            "dsf" => Ok(AudioFileType::Dsf),
            "m4a" => Ok(AudioFileType::M4a),
            "opus" => Ok(AudioFileType::Opus),
            "ra" => Ok(AudioFileType::Ra),
            "au" => Ok(AudioFileType::Au),
            "aiff" => Ok(AudioFileType::Aiff),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum VideoFileType {
    Mp4,
    Mkv,
    Avi,
    Flv,
    Mov,
    Wmv,
    Mpeg,
    Webm,
    Ogv,
    Vob,
    M4v,
    Mpg,
    Other(String),
}

impl fmt::Display for VideoFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                VideoFileType::Mp4 => "mp4",
                VideoFileType::Mkv => "mkv",
                VideoFileType::Avi => "avi",
                VideoFileType::Flv => "flv",
                VideoFileType::Mov => "mov",
                VideoFileType::Wmv => "wmv",
                VideoFileType::Mpeg => "mpeg",
                VideoFileType::Webm => "webm",
                VideoFileType::Ogv => "ogv",
                VideoFileType::Vob => "vob",
                VideoFileType::M4v => "m4v",
                VideoFileType::Mpg => "mpg",
                VideoFileType::Other(s) => s.as_str(),
            }
        )
    }
}

impl FromStr for VideoFileType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mp4" => Ok(VideoFileType::Mp4),
            "mkv" => Ok(VideoFileType::Mkv),
            "avi" => Ok(VideoFileType::Avi),
            "flv" => Ok(VideoFileType::Flv),
            "mov" => Ok(VideoFileType::Mov),
            "wmv" => Ok(VideoFileType::Wmv),
            "mpeg" => Ok(VideoFileType::Mpeg),
            "webm" => Ok(VideoFileType::Webm),
            "ogv" => Ok(VideoFileType::Ogv),
            "vob" => Ok(VideoFileType::Vob),
            "m4v" => Ok(VideoFileType::M4v),
            "mpg" => Ok(VideoFileType::Mpg),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ShinkaiFileType {
    ShinkaiJobExtension,
    ShinkaiVectorResource,
    ShinkaiResourceRouter,
    Other(String),
}

impl fmt::Display for ShinkaiFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ShinkaiFileType::ShinkaiJobExtension => "jobkai",
                ShinkaiFileType::ShinkaiVectorResource => "vrkai",
                ShinkaiFileType::ShinkaiResourceRouter => "routerkai",
                ShinkaiFileType::Other(s) => s.as_str(),
            }
        )
    }
}

impl FromStr for ShinkaiFileType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jobkai" => Ok(ShinkaiFileType::ShinkaiJobExtension),
            "vrkai" => Ok(ShinkaiFileType::ShinkaiVectorResource),
            "routerkai" => Ok(ShinkaiFileType::ShinkaiResourceRouter),
            _ => Err(()),
        }
    }
}
