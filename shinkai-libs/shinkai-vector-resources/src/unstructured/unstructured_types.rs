use serde::Deserialize;

/// An intermediary type in between `UnstructuredElement`s and
/// `Embedding`s/`DataChunk`s
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GroupedText {
    pub text: String,
    pub page_numbers: Vec<u32>,
    pub sub_groups: Vec<GroupedText>,
}

impl GroupedText {
    pub fn new() -> Self {
        GroupedText {
            text: String::new(),
            page_numbers: Vec::new(),
            sub_groups: Vec::new(),
        }
    }

    /// Pushes data into this GroupedText
    pub fn push_data(&mut self, text: &str, page_number: Option<u32>) {
        if !self.text.is_empty() {
            self.text.push(' ');
        }
        self.text.push_str(text);

        if let Some(page_number) = page_number {
            if !self.page_numbers.contains(&page_number) {
                self.page_numbers.push(page_number);
            }
        }
    }

    /// Pushes a sub-group into this GroupedText
    pub fn push_sub_group(&mut self, sub_group: GroupedText) {
        self.sub_groups.push(sub_group);
    }

    /// Outputs a String that holds an array of the page numbers
    pub fn format_page_num_string(&self) -> String {
        format!(
            "[{}]",
            self.page_numbers
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

/// Different types of elements Unstructured can output
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub enum ElementType {
    Title,
    NarrativeText,
    UncategorizedText,
    ListItem,
    EmailAddress,
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
    pub link_texts: Option<Vec<String>>,
    pub links: Option<Vec<Link>>,
    pub section: Option<String>,
    pub parent_id: Option<String>,
    pub category_depth: Option<u32>,
    pub text_as_html: Option<String>,
    pub languages: Option<Vec<String>>,
    pub emphasized_text_contents: Option<String>,
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
