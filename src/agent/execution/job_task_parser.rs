use serde::{Deserialize, Serialize};

/// Represents an analyzed/parsed initial message which triggered the job to run (aka. job task)
/// Holds an ordered list of elements, which are pieces of the original job task string with parsed metadata about them
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedJobTask {
    pub original_job_task_string: String,
    pub elements: Vec<JobTaskElement>,
}

impl ParsedJobTask {
    pub fn new(original_job_task_string: String) -> Self {
        // Clean the original job task string by removing trailing newlines/whitespace
        let original_job_task_string = original_job_task_string.trim_end_matches('\n').trim().to_string();
        let elements = Self::parse_original_job_task_string(&original_job_task_string);
        ParsedJobTask {
            original_job_task_string,
            elements,
        }
    }

    /// Creates a new `ParsedJobTask` using the given elements (recreates original job task string)
    pub fn new_from_elements(elements: Vec<JobTaskElement>) -> Self {
        let orig_text = elements
            .iter()
            .map(|element| match element {
                JobTaskElement::Text(text) => text.content.clone(),
                JobTaskElement::CodeBlock(code_block) => code_block.get_output_string(),
                JobTaskElement::ListPoint(list_point) => list_point.get_output_string(),
            })
            .collect::<Vec<String>>()
            .join(" ");
        ParsedJobTask {
            original_job_task_string: orig_text,
            elements,
        }
    }

    /// Parses the original job task string into a list of job task elements
    fn parse_original_job_task_string(original_job_task_string: &str) -> Vec<JobTaskElement> {
        let mut elements = Vec::new();

        // Split the text elements from the codeblocks
        let split_text_on_code_blocks = split_text_on_code_blocks(original_job_task_string);
        for text in split_text_on_code_blocks {
            if text.starts_with("```") {
                // TODO: process language eventually
                elements.push(JobTaskElement::CodeBlock(CodeBlockTaskElement::new(text, None)));
            } else {
                elements.push(JobTaskElement::Text(TextTaskElement::new(text)));
            }
        }

        // Parses the list point elements out of the text elements, and preserves ordering
        let elements = parse_list_point_elements_from_text_elements(elements);

        elements
    }

    /// Returns a reference to the elements of the job task
    pub fn get_elements(&self) -> &Vec<JobTaskElement> {
        &self.elements
    }

    /// Returns a copy of the elements of the job task
    pub fn get_elements_cloned(&self) -> Vec<JobTaskElement> {
        self.elements.clone()
    }

    /// Returns a copy of the elements of the job task, filtered by the given parameters
    pub fn get_elements_filtered(&self, exclude_text: bool, exclude_code_blocks: bool) -> Vec<JobTaskElement> {
        self.elements
            .iter()
            .filter_map(|element| match element {
                JobTaskElement::Text(_) if !exclude_text => Some(element.clone()),
                JobTaskElement::CodeBlock(_) if !exclude_code_blocks => Some(element.clone()),
                _ => None,
            })
            .collect()
    }

    /// Returns a string representation of the job task.
    /// Currently should be equivalent to the original job task string, but in future extra parsed
    /// data may be added.
    pub fn get_output_string(&self) -> String {
        self.elements
            .iter()
            .map(|element| match element {
                JobTaskElement::Text(text) => format!("{} ", text.content.clone()),
                JobTaskElement::CodeBlock(code_block) => format!("{} ", code_block.content.clone()),
                JobTaskElement::ListPoint(list_point) => format!("{} ", list_point.get_output_string()),
            })
            .collect::<Vec<String>>()
            .join("")
            .trim()
            .to_string()
    }

    /// Returns a string representation of the job task, filtered by the given parameters
    pub fn get_output_string_filtered(&self, exclude_text: bool, exclude_code_blocks: bool) -> String {
        let filtered_elements = self.get_elements_filtered(exclude_text, exclude_code_blocks);
        ParsedJobTask::new_from_elements(filtered_elements).get_output_string()
    }
}

/// A parsed element from the original job task string
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobTaskElement {
    Text(TextTaskElement),
    CodeBlock(CodeBlockTaskElement),
    ListPoint(ListPoint),
    List(ListTaskElement),
}

impl JobTaskElement {
    /// Returns the length of the underlying text or code block
    pub fn content_len(&self) -> usize {
        match self {
            JobTaskElement::Text(text_element) => text_element.content_len(),
            JobTaskElement::CodeBlock(code_block_element) => code_block_element.content_len(),
            JobTaskElement::ListPoint(list_point_element) => list_point_element.content_len(),
            JobTaskElement::List(list_element) => list_element.content_len(),
        }
    }
}

/// A piece of text from the original job task string
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextTaskElement {
    pub content: String,
}

impl TextTaskElement {
    /// Creates a new `TextTaskElement`
    pub fn new(text: String) -> Self {
        TextTaskElement { content: text }
    }

    /// Returns the length of the text
    pub fn content_len(&self) -> usize {
        self.content.len()
    }
}

/// A code block from the original job task string
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeBlockTaskElement {
    pub content: String,
    pub language: Option<String>,
}

impl CodeBlockTaskElement {
    /// Creates a new `CodeBlockTaskElement`
    pub fn new(code_block: String, language: Option<String>) -> Self {
        CodeBlockTaskElement {
            content: code_block,
            language,
        }
    }

    /// Returns the length of the code block
    pub fn content_len(&self) -> usize {
        self.content.len()
    }

    /// Returns a string representation of the code block
    pub fn get_output_string(&self) -> String {
        format!("{}", self.content)
    }
}

/// Represents a list item in a job task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListPoint {
    pub content: String,
}

impl ListPoint {
    /// Creates a new `ListTaskElement`
    pub fn new(content: String) -> Self {
        ListPoint { content }
    }

    /// Returns the length of the list point content
    pub fn content_len(&self) -> usize {
        self.content.len()
    }

    /// Returns a string representation of the code block
    pub fn get_output_string(&self) -> String {
        format!("\n- {}", self.content)
    }
}

/// Represents a list item in a job task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListTaskElement {
    pub list_points: Vec<ListPoint>,
}

impl ListTaskElement {
    /// Creates a new `ListTaskElement`
    pub fn new(content: String) -> Self {
        ListPoint { content }
    }

    /// Returns the number of list points in the list
    pub fn len(&self) -> usize {
        self.list_points.len()
    }
    /// Returns the length of the list point content
    pub fn content_len(&self) -> usize {
        self.list_points.iter().map(|list_point| list_point.content_len()).sum()
    }

    /// Returns a string representation of the code block
    pub fn get_output_string(&self) -> String {
        format!("\n- {}", self.content)
    }
}

/// Splits the text into segments based on code blocks delineated by triple backticks.
/// Code block strings start and end with triple backticks, while other strings do not contain them.
fn split_text_on_code_blocks(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current_segment = String::new();
    let mut in_code_block = false;
    let mut backtick_sequence = 0;

    for c in text.chars() {
        current_segment.push(c);

        if c == '`' {
            backtick_sequence += 1;
        } else {
            backtick_sequence = 0;
        }

        // When encountering three backticks, toggle the in_code_block flag
        if backtick_sequence == 3 {
            if in_code_block {
                // End of a code block, push the segment including the closing backticks
                segments.push(current_segment);
                current_segment = String::new();
            } else {
                // Start of a code block, remove the opening backticks from the previous segment
                // and prepare the new segment starting with backticks
                if !current_segment.is_empty() {
                    let non_code_segment = current_segment[..current_segment.len() - 3].to_string();
                    if !non_code_segment.trim().is_empty() {
                        segments.push(non_code_segment);
                    }
                }
                current_segment = "```".to_string();
            }
            in_code_block = !in_code_block;
            backtick_sequence = 0; // Reset backtick sequence after processing
        }
    }

    // Add the last segment if it's not empty
    if !current_segment.trim().is_empty() {
        segments.push(current_segment);
    }

    segments
}

/// Generates list item patterns including '-', '*', and '1.' to '20.' with various spacings
fn get_list_item_patterns() -> Vec<String> {
    let mut patterns = Vec::new();

    // Patterns for unordered lists
    let unordered_markers = vec!["-", "*"];

    // Generate patterns for unordered list markers with various spacings
    for marker in unordered_markers.iter() {
        patterns.extend(vec![
            format!("\n{} ", marker),
            format!("\n {} ", marker),
            format!("\n  {} ", marker),
            format!(". {} ", marker),
            format!(".  {} ", marker),
        ]);
    }

    // Generate patterns for ordered list numbers 1 to 20 with various spacings
    for i in 1..=20 {
        patterns.extend(vec![
            format!("\n{}. ", i),
            format!("\n {}. ", i),
            format!("\n  {}. ", i),
        ]);
    }

    patterns
}

/// Parses list elements from text elements and intersperses them in the original order
fn parse_list_point_elements_from_text_elements(elements: Vec<JobTaskElement>) -> Vec<JobTaskElement> {
    let mut new_elements = Vec::new();
    let list_item_patterns = get_list_item_patterns();

    for element in elements.into_iter() {
        match element {
            JobTaskElement::Text(text_element) => {
                let mut text = text_element.content.clone();
                let mut last_pos = 0;
                let mut list_items = Vec::new();

                for pattern in &list_item_patterns {
                    let mut pos = text[last_pos..].find(pattern);
                    while pos.is_some() {
                        let start = last_pos + pos.unwrap();
                        let end = text[start..].find('\n').map_or(text.len(), |p| start + p);
                        // Adjust to extract the list item content correctly, trimming the pattern if necessary
                        let item_content = text[start + pattern.len()..end].trim().to_string();
                        list_items.push((start, item_content));
                        last_pos = end;
                        pos = text[last_pos..].find(pattern);
                    }
                }

                list_items.sort_by(|a, b| a.0.cmp(&b.0));
                let mut current_pos = 0;

                for (start, item_content) in list_items {
                    if start > current_pos {
                        // Push text before the list item as a new Text element
                        new_elements.push(JobTaskElement::Text(TextTaskElement::new(
                            text[current_pos..start].to_string(),
                        )));
                    }
                    // Push the list item as a new ListPoint element
                    new_elements.push(JobTaskElement::ListPoint(ListPoint::new(item_content.clone())));
                    current_pos = start + item_content.len() + 1; // Adjust for the newline character
                }

                // If there's remaining text after the last list item, add it as a new Text element
                if current_pos < text.len() {
                    new_elements.push(JobTaskElement::Text(TextTaskElement::new(
                        text[current_pos..].to_string(),
                    )));
                }
            }
            _ => new_elements.push(element),
        }
    }

    new_elements
}
