use std::any::Any;
use std::fmt;

use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::vector_resource::VRPath;

use super::argument::ToolOutputArg;

#[derive(Debug)]
pub enum RustToolError {
    InvalidFunctionArguments(String),
    FailedJSONParsing,
}

impl fmt::Display for RustToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RustToolError::InvalidFunctionArguments(msg) => write!(f, "Invalid function arguments: {}", msg),
            RustToolError::FailedJSONParsing => write!(f, "Failed to parse JSON"),
        }
    }
}

impl std::error::Error for RustToolError {}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RustTool {
    pub name: String,
    pub description: String,
    pub input_args: Vec<ToolArgument>,
    pub output_arg: ToolOutputArg,
    pub tool_embedding: Option<Embedding>,
}

impl RustTool {
    pub fn new(
        name: String,
        description: String,
        input_args: Vec<ToolArgument>,
        tool_embedding: Option<Embedding>,
    ) -> Self {
        Self {
            name: VRPath::clean_string(&name),
            description,
            input_args,
            output_arg: ToolOutputArg { json: "".to_string() },
            tool_embedding,
        }
    }

    /// Default name of the rust toolkit
    pub fn toolkit_name(&self) -> String {
        "rust-toolkit".to_string()
    }

    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Convert from json
    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json)?;
        Ok(deserialized)
    }
}

impl RustTool {
    pub async fn static_tools() -> Vec<Self> {
        let mut tools = Vec::new();

        tools.push(RustTool::new(
            "concat_strings".to_string(),
            "Concatenates 2 to 4 strings.".to_string(),
            vec![
                ToolArgument::new(
                    "first_string".to_string(),
                    "string".to_string(),
                    "The first string to concatenate".to_string(),
                    true,
                ),
                ToolArgument::new(
                    "second_string".to_string(),
                    "string".to_string(),
                    "The second string to concatenate".to_string(),
                    true,
                ),
                ToolArgument::new(
                    "third_string".to_string(),
                    "string".to_string(),
                    "The third string to concatenate (optional)".to_string(),
                    false,
                ),
                ToolArgument::new(
                    "fourth_string".to_string(),
                    "string".to_string(),
                    "The fourth string to concatenate (optional)".to_string(),
                    false,
                ),
            ],
            None,
        ));

        tools.push(RustTool::new(
            "search_and_replace".to_string(),
            "Searches for a substring and replaces it with another substring.".to_string(),
            vec![
                ToolArgument::new(
                    "text".to_string(),
                    "string".to_string(),
                    "The text to search in".to_string(),
                    true,
                ),
                ToolArgument::new(
                    "search".to_string(),
                    "string".to_string(),
                    "The substring to search for".to_string(),
                    true,
                ),
                ToolArgument::new(
                    "replace".to_string(),
                    "string".to_string(),
                    "The substring to replace with".to_string(),
                    true,
                ),
            ],
            None,
        ));

        tools.push(RustTool::new(
            "download_webpage".to_string(),
            "Downloads the content of a webpage.".to_string(),
            vec![ToolArgument::new(
                "url".to_string(),
                "string".to_string(),
                "The URL of the webpage to download".to_string(),
                true,
            )],
            None,
        ));

        tools.push(RustTool::new(
            "html_to_markdown".to_string(),
            "Converts HTML content to Markdown.".to_string(),
            vec![ToolArgument::new(
                "html_content".to_string(),
                "string".to_string(),
                "The HTML content to convert".to_string(),
                true,
            )],
            None,
        ));

        tools.push(RustTool::new(
            "array_to_markdown_template".to_string(),
            "Converts a comma-separated string to a Markdown template.".to_string(),
            vec![ToolArgument::new(
                "comma_separated_string".to_string(),
                "string".to_string(),
                "The comma-separated string to convert".to_string(),
                true,
            )],
            None,
        ));

        tools.push(RustTool::new(
            "fill_variable_in_md_template".to_string(),
            "Fills a variable in a Markdown template.".to_string(),
            vec![
                ToolArgument::new(
                    "markdown_template".to_string(),
                    "string".to_string(),
                    "The Markdown template".to_string(),
                    true,
                ),
                ToolArgument::new(
                    "variable_name".to_string(),
                    "string".to_string(),
                    "The variable name to fill".to_string(),
                    true,
                ),
                ToolArgument::new(
                    "content".to_string(),
                    "string".to_string(),
                    "The content to fill in the template".to_string(),
                    true,
                ),
            ],
            None,
        ));

        // let print_arg_desc = "Prints a single argument.".to_string();
        // tools.push(RustTool::new(
        //     "print_arg".to_string(),
        //     print_arg_desc.clone(),
        //     vec![ToolArgument::new(
        //         "argument".to_string(),
        //         "string".to_string(),
        //         "The argument to print".to_string(),
        //         true,
        //     )],
        //     generator.generate_embedding_default(&print_arg_desc).await.unwrap(),
        // ));

        tools.push(RustTool::new(
            "return_error_message".to_string(),
            "The error message to return. Useful for debugging in workflows.".to_string(),
            vec![ToolArgument::new(
                "error_message".to_string(),
                "string".to_string(),
                "The error message to return. Useful for debugging in workflows.".to_string(),
                true,
            )],
            None,
        ));

        tools.push(RustTool::new(
            "count_files_from_input".to_string(),
            "Counts files with a specific extension.".to_string(),
            vec![ToolArgument::new(
                "file_extension".to_string(),
                "string".to_string(),
                "The file extension to count (optional)".to_string(),
                false,
            )],
            None,
        ));

        tools.push(RustTool::new(
            "retrieve_file_from_input".to_string(),
            "Retrieves a file by name.".to_string(),
            vec![ToolArgument::new(
                "filename".to_string(),
                "string".to_string(),
                "The filename to retrieve".to_string(),
                true,
            )],
            None,
        ));

        tools.push(RustTool::new(
            "extract_and_map_csv_column".to_string(),
            "Extracts and maps a CSV column.".to_string(),
            vec![
                ToolArgument::new(
                    "csv_data".to_string(),
                    "Vec<u8>".to_string(),
                    "The CSV data to extract and map".to_string(),
                    true,
                ),
                ToolArgument::new(
                    "column_identifier".to_string(),
                    "string".to_string(),
                    "The column identifier".to_string(),
                    true,
                ),
                ToolArgument::new(
                    "map_function".to_string(),
                    "Box<dyn Fn(&str) -> String + Send>".to_string(),
                    "The map function".to_string(),
                    true,
                ),
            ],
            None,
        ));

        tools.push(RustTool::new(
            "process_embeddings_in_job_scope".to_string(),
            "Processes embeddings in job scope.".to_string(),
            vec![ToolArgument::new(
                "map_function".to_string(),
                "Box<dyn Fn(&str) -> String + Send + Sync>".to_string(),
                "The map function".to_string(),
                true,
            )],
            None,
        ));

        tools
    }

    pub fn convert_args_from_fn_call(
        function_args: serde_json::Map<String, serde_json::Value>,
    ) -> Result<Vec<Box<dyn Any + Send + 'static>>, RustToolError> {
        function_args
            .into_iter()
            .map(|(_, value)| match value {
                serde_json::Value::String(s) => Ok(Box::new(s) as Box<dyn Any + Send>),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        Ok(Box::new(i) as Box<dyn Any + Send>)
                    } else if let Some(f) = n.as_f64() {
                        Ok(Box::new(f) as Box<dyn Any + Send>)
                    } else {
                        Ok(Box::new(n.to_string()) as Box<dyn Any + Send>)
                    }
                }
                serde_json::Value::Bool(b) => Ok(Box::new(b) as Box<dyn Any + Send>),
                _ => Ok(Box::new(value.to_string()) as Box<dyn Any + Send>),
            })
            .collect()
    }
}
