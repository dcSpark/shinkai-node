use super::DistributionInfo;

use crate::resource_errors::VRError;
use crate::source::notary_source::{NotarizedSourceReference, TLSNotarizedSourceFile, TLSNotaryProof};

use regex::Regex;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use utoipa::ToSchema;

/// What text chunking strategy was used to create this VR from the source file.
/// This is required for performing content validation/that it matches the VR nodes.
/// TODO: Think about how to make this more explicit/specific and future support
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub enum TextChunkingStrategy {
    /// The default text chunking strategy implemented in VR lib using local parsing.
    V1,
}

/// Information about the source content a Vector Resource came from
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub enum VRSourceReference {
    Standard(SourceReference),
    Notarized(NotarizedSourceReference),
    None,
}

impl VRSourceReference {
    /// Formats a printable string based on the source
    pub fn format_source_string(&self) -> String {
        match self {
            VRSourceReference::Standard(reference) => reference.format_source_string(),
            VRSourceReference::Notarized(notarized_reference) => notarized_reference.format_source_string(),
            VRSourceReference::None => String::from("None"),
        }
    }

    /// Creates a VRSourceReference from an external URI or URL
    pub fn new_uri_ref(uri: &str) -> Self {
        Self::Standard(SourceReference::new_external_uri(uri.to_string()))
    }

    /// Creates a VRSourceReference reference to an original source file
    pub fn new_source_file_ref(
        file_name: String,
        file_type: SourceFileType,
        text_chunking_strategy: TextChunkingStrategy,
    ) -> Self {
        VRSourceReference::Standard(SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type,
            text_chunking_strategy,
        }))
    }

    /// Creates a VRSourceReference reference using an arbitrary String
    pub fn new_other_ref(other: &str) -> Self {
        Self::Standard(SourceReference::new_other(other.to_string()))
    }

    /// Creates a VRSourceReference which represents no/unknown source.
    pub fn none() -> Self {
        VRSourceReference::None
    }

    /// Serializes the VRSourceReference to a JSON string
    pub fn to_json(&self) -> Result<String, VRError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserializes a VRSourceReference from a JSON string
    pub fn from_json(json: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Creates a VRSourceReference using file_name/content to auto-detect and create an instance of Self.
    /// Errors if can not detect matching extension in file_name.
    pub fn from_file(file_name: &str, text_chunking_strategy: TextChunkingStrategy) -> Result<Self, VRError> {
        let file_name_without_extension = SourceFileType::clean_string_of_extension(file_name);
        // Attempt to auto-detect, else use file extension
        let file_type = SourceFileType::detect_file_type(file_name)?;
        if file_name.starts_with("http") {
            Ok(VRSourceReference::new_uri_ref(file_name))
        } else {
            let file_name_without_extension = file_name_without_extension.trim_start_matches("file://");
            Ok(VRSourceReference::new_source_file_ref(
                file_name_without_extension.to_string(),
                file_type,
                text_chunking_strategy,
            ))
        }
    }

    /// Checks if the VRSourceReference is of type Standard
    pub fn is_standard(&self) -> bool {
        matches!(self, VRSourceReference::Standard(_))
    }

    /// Checks if the VRSourceReference is of type Notarized
    pub fn is_notarized(&self) -> bool {
        matches!(self, VRSourceReference::Notarized(_))
    }

    /// Checks if the VRSourceReference is of type None
    pub fn is_none(&self) -> bool {
        matches!(self, VRSourceReference::None)
    }
}

/// Struct which holds the data of a source file which a VR was generated from
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub enum SourceFile {
    Standard(StandardSourceFile),
    TLSNotarized(TLSNotarizedSourceFile),
}

impl SourceFile {
    pub fn new_standard_source_file(
        file_name: String,
        file_type: SourceFileType,
        file_content: Vec<u8>,
        distribution_info: Option<DistributionInfo>,
    ) -> Self {
        Self::Standard(StandardSourceFile {
            file_name,
            file_type,
            file_content,
            distribution_info,
        })
    }

    pub fn new_tls_notarized_source_file(
        file_name: String,
        file_type: SourceFileType,
        file_content: Vec<u8>,
        distribution_info: Option<DistributionInfo>,
        proof: TLSNotaryProof,
    ) -> Self {
        Self::TLSNotarized(TLSNotarizedSourceFile {
            file_name,
            file_type,
            file_content,
            distribution_info,
            proof,
        })
    }

    /// Serializes the SourceFile to a JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserializes a SourceFile from a JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
/// A standard source file that data was extracted from to create a VectorResource.
pub struct StandardSourceFile {
    pub file_name: String,
    pub file_type: SourceFileType,
    pub file_content: Vec<u8>,
    // Creation/publication time of the original content which is inside this struct
    pub distribution_info: Option<DistributionInfo>,
}

impl StandardSourceFile {
    /// Returns the size of the file content in bytes
    pub fn size(&self) -> usize {
        self.file_content.len()
    }

    /// Creates a new instance of SourceFile struct
    pub fn new(
        file_name: String,
        file_type: SourceFileType,
        file_content: Vec<u8>,
        distribution_info: Option<DistributionInfo>,
    ) -> Self {
        Self {
            file_name,
            file_type,
            file_content,
            distribution_info,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
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
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct ExternalURIReference {
    pub uri: String,
}

impl SourceReference {
    pub fn format_source_string(&self) -> String {
        match self {
            SourceReference::FileRef(reference) => reference.format_source_string(),
            SourceReference::ExternalURI(uri) => uri.uri.clone(),
            SourceReference::Other(s) => s.clone(),
        }
    }

    /// Creates a new SourceReference for a file, auto-detecting the file type
    /// by attempting to parse the extension in the file_name.
    /// Errors if extension is not found or not implemented yet.
    pub fn new_file_reference_auto_detect(
        file_name: String,
        text_chunking_strategy: TextChunkingStrategy,
    ) -> Result<Self, VRError> {
        let file_type = SourceFileType::detect_file_type(&file_name)?;
        Ok(SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type,
            text_chunking_strategy,
        }))
    }

    /// Creates a new SourceReference for an image file
    pub fn new_file_image_reference(
        file_name: String,
        image_type: ImageFileType,
        text_chunking_strategy: TextChunkingStrategy,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Image(image_type),
            text_chunking_strategy,
        })
    }

    /// Creates a new SourceReference for a document file
    pub fn new_file_doc_reference(
        file_name: String,
        doc_type: DocumentFileType,
        text_chunking_strategy: TextChunkingStrategy,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Document(doc_type),
            text_chunking_strategy,
        })
    }

    /// Creates a new SourceReference for a code file
    pub fn new_file_code_reference(
        file_name: String,
        code_type: CodeFileType,
        text_chunking_strategy: TextChunkingStrategy,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Code(code_type),
            text_chunking_strategy,
        })
    }

    /// Creates a new SourceReference for a config file
    pub fn new_file_config_reference(
        file_name: String,
        config_type: ConfigFileType,
        text_chunking_strategy: TextChunkingStrategy,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::ConfigFileType(config_type),
            text_chunking_strategy,
        })
    }

    /// Creates a new SourceReference for a video file
    pub fn new_file_video_reference(
        file_name: String,
        video_type: VideoFileType,
        text_chunking_strategy: TextChunkingStrategy,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Video(video_type),
            text_chunking_strategy,
        })
    }

    /// Creates a new SourceReference for an audio file
    pub fn new_file_audio_reference(
        file_name: String,
        audio_type: AudioFileType,
        text_chunking_strategy: TextChunkingStrategy,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Audio(audio_type),
            text_chunking_strategy,
        })
    }

    /// Creates a new SourceReference for a Shinkai file
    pub fn new_file_shinkai_reference(
        file_name: String,
        shinkai_type: ShinkaiFileType,
        text_chunking_strategy: TextChunkingStrategy,
    ) -> Self {
        SourceReference::FileRef(SourceFileReference {
            file_name,
            file_type: SourceFileType::Shinkai(shinkai_type),
            text_chunking_strategy,
        })
    }

    /// Creates a new SourceReference for an external URI
    pub fn new_external_uri(uri: String) -> Self {
        SourceReference::ExternalURI(ExternalURIReference { uri })
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
            SourceReference::ExternalURI(uri) => write!(f, "{}", uri.uri),
            SourceReference::Other(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct SourceFileReference {
    pub file_name: String,
    pub file_type: SourceFileType,
    pub text_chunking_strategy: TextChunkingStrategy,
}

impl SourceFileReference {
    pub fn format_source_string(&self) -> String {
        format!("{}.{}", self.file_name, self.file_type)
    }
}

impl fmt::Display for SourceFileReference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "File Name: {}, File Type: {}", self.file_name, self.file_type)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
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
        let path = Path::new(file_name);
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| VRError::FileTypeNotSupported(file_name.to_string()))?;

        let ext = extension;
        {
            if let Ok(doc_type) = DocumentFileType::from_str(ext) {
                return Ok(SourceFileType::Document(doc_type));
            }
            if let Ok(code_type) = CodeFileType::from_str(ext) {
                return Ok(SourceFileType::Code(code_type));
            }
            // Config support will be added once we implement parsers for them all
            if let Ok(_config_type) = ConfigFileType::from_str(ext) {
                // return Ok(SourceFileType::ConfigFileType(config_type));
                return Err(VRError::FileTypeNotSupported(file_name.to_string()));
            }
            if let Ok(shinkai_type) = ShinkaiFileType::from_str(ext) {
                return Ok(SourceFileType::Shinkai(shinkai_type));
            }
            // Video/audio/image support will come in the future by first converting to text.
            if let Ok(_video_type) = VideoFileType::from_str(ext) {
                // return Ok(SourceFileType::Video(video_type));
                return Err(VRError::FileTypeNotSupported(file_name.to_string()));
            }
            if let Ok(_audio_type) = AudioFileType::from_str(ext) {
                // return Ok(SourceFileType::Audio(audio_type));
                return Err(VRError::FileTypeNotSupported(file_name.to_string()));
            }
            if let Ok(_img_type) = ImageFileType::from_str(ext) {
                // return Ok(SourceFileType::Image(img_type));
                return Err(VRError::FileTypeNotSupported(file_name.to_string()));
            }
        }

        Err(VRError::FileTypeNotSupported(file_name.to_string()))
    }

    /// Clones and cleans the input string of its file extension at the end, if it exists.
    pub fn clean_string_of_extension(file_name: &str) -> String {
        // If the file extension is not detected/supported, return the original file_name
        if SourceFileType::detect_file_type(file_name).is_err() {
            file_name.to_string()
        } else {
            let re = Regex::new(r"\.[^.]+$").unwrap();
            let file_name_without_extension = re.replace(file_name, "").to_string();

            // Check if the result is empty, return the original file_name if so as a backup
            if file_name_without_extension.is_empty() {
                file_name.to_string()
            } else {
                file_name_without_extension
            }
        }
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
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
            "ps" => Ok(DocumentFileType::Ps),
            "tex" => Ok(DocumentFileType::Tex),
            "latex" => Ok(DocumentFileType::Latex),
            "ods" => Ok(DocumentFileType::Ods),
            "odp" => Ok(DocumentFileType::Odp),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub enum ConfigFileType {
    Toml,
    Ini,
    Yaml,
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
                ConfigFileType::Yaml => "yaml",
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
            "yaml" => Ok(ConfigFileType::Yaml),
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub enum ShinkaiFileType {
    ShinkaiJobKai,
    ShinkaiVRKai,
    ShinkaiVRPack,
    Other(String),
}

impl fmt::Display for ShinkaiFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ShinkaiFileType::ShinkaiJobKai => "jobkai",
                ShinkaiFileType::ShinkaiVRKai => "vrkai",
                ShinkaiFileType::ShinkaiVRPack => "vrpack",
                ShinkaiFileType::Other(s) => s.as_str(),
            }
        )
    }
}

impl FromStr for ShinkaiFileType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jobkai" => Ok(ShinkaiFileType::ShinkaiJobKai),
            "vrkai" => Ok(ShinkaiFileType::ShinkaiVRKai),
            "vrpack" => Ok(ShinkaiFileType::ShinkaiVRPack),
            _ => Err(()),
        }
    }
}
