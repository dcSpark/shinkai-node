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
                JobTaskElement::CodeBlock(code_block) => code_block.content.clone(),
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
}

impl JobTaskElement {
    /// Returns the length of the underlying text or code block
    pub fn len(&self) -> usize {
        match self {
            JobTaskElement::Text(text_element) => text_element.len(),
            JobTaskElement::CodeBlock(code_block_element) => code_block_element.len(),
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
    pub fn len(&self) -> usize {
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
    pub fn len(&self) -> usize {
        self.content.len()
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
