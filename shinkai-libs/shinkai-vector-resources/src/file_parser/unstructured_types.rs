use serde::Deserialize;

/// Different types of elements Unstructured can output
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub enum ElementType {
    FigureCaption,
    NarrativeText,
    ListItem,
    Title,
    Address,
    Table,
    PageBreak,
    Header,
    Footer,
    UncategorizedText,
    Image,
    Formula,
    EmailAddress,
    CompositeElement,
    TableChunk, // Specialized form of Table
    SectionHeader,
    Headline,
    SubHeadline,
    FieldName,
    Text, // General text, might overlap with NarrativeText
    Abstract,
    Threading,
    Form,
    Value,
    Link,
    BulletedText,
    ListItemOther,
    PageHeader,
    PageFooter,
    Footnote,
    Caption, // Might overlap with FigureCaption
    Figure,
    Picture,
}

/// Output data from Unstructured which holds a piece of text and
/// relevant data.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UnstructuredElement {
    #[serde(rename = "type")]
    pub element_type: ElementType,
    pub element_id: String,
    pub metadata: Metadata,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Metadata {
    pub filename: String,
    pub file_directory: Option<String>,
    pub last_modified: Option<String>,
    pub filetype: String,
    pub coordinates: Option<Vec<f32>>,
    pub page_number: Option<u32>,
    pub page_name: Option<String>,
    pub sent_from: Option<String>,
    pub sent_to: Option<String>,
    pub subject: Option<String>,
    pub attached_to_filename: Option<String>,
    pub header_footer_type: Option<String>,
    pub link_urls: Option<Vec<String>>,
    pub link_texts: Option<Vec<Option<String>>>,
    pub links: Option<Vec<Link>>,
    pub section: Option<String>,
    pub parent_id: Option<String>,
    pub category_depth: Option<u32>,
    pub text_as_html: Option<String>,
    pub languages: Option<Vec<String>>,
    pub emphasized_text_contents: Option<Vec<String>>,
    pub emphasized_text_tags: Option<Vec<String>>,
    pub num_characters: Option<u32>,
    pub is_continuation: Option<bool>,
    pub detection_class_prob: Option<Vec<f32>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Link {
    text: String,
    url: String,
}
