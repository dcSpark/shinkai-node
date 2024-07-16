use std::any::Any;

use crate::llm_provider::error::LLMProviderError;
use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::vector_resource::VRPath;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RustTool {
    pub name: String,
    pub description: String,
    pub input_args: Vec<ToolArgument>,
    pub tool_embedding: Embedding,
}

impl RustTool {
    pub fn new(name: String, description: String, input_args: Vec<ToolArgument>, tool_embedding: Embedding) -> Self {
        Self {
            name: VRPath::clean_string(&name),
            description,
            input_args,
            tool_embedding,
        }
    }

    /// Default name of the rust toolkit
    pub fn toolkit_type_name(&self) -> String {
        self.name.clone()
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
    pub async fn static_tools(generator: Box<dyn EmbeddingGenerator>) -> Vec<Self> {
        let mut tools = Vec::new();

        let concat_strings_desc = "Concatenates 2 to 4 strings.".to_string();
        tools.push(RustTool::new(
            "concat_strings".to_string(),
            concat_strings_desc.clone(),
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
            generator
                .generate_embedding_default(&concat_strings_desc)
                .await
                .unwrap(),
        ));

        let search_and_replace_desc = "Searches for a substring and replaces it with another substring.".to_string();
        tools.push(RustTool::new(
            "search_and_replace".to_string(),
            search_and_replace_desc.clone(),
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
            generator
                .generate_embedding_default(&search_and_replace_desc)
                .await
                .unwrap(),
        ));

        let download_webpage_desc = "Downloads the content of a webpage.".to_string();
        tools.push(RustTool::new(
            "download_webpage".to_string(),
            download_webpage_desc.clone(),
            vec![ToolArgument::new(
                "url".to_string(),
                "string".to_string(),
                "The URL of the webpage to download".to_string(),
                true,
            )],
            generator
                .generate_embedding_default(&download_webpage_desc)
                .await
                .unwrap(),
        ));

        let html_to_markdown_desc = "Converts HTML content to Markdown.".to_string();
        tools.push(RustTool::new(
            "html_to_markdown".to_string(),
            html_to_markdown_desc.clone(),
            vec![ToolArgument::new(
                "html_content".to_string(),
                "string".to_string(),
                "The HTML content to convert".to_string(),
                true,
            )],
            generator
                .generate_embedding_default(&html_to_markdown_desc)
                .await
                .unwrap(),
        ));

        let array_to_markdown_template_desc = "Converts a comma-separated string to a Markdown template.".to_string();
        tools.push(RustTool::new(
            "array_to_markdown_template".to_string(),
            array_to_markdown_template_desc.clone(),
            vec![ToolArgument::new(
                "comma_separated_string".to_string(),
                "string".to_string(),
                "The comma-separated string to convert".to_string(),
                true,
            )],
            generator
                .generate_embedding_default(&array_to_markdown_template_desc)
                .await
                .unwrap(),
        ));

        let fill_variable_in_md_template_desc = "Fills a variable in a Markdown template.".to_string();
        tools.push(RustTool::new(
            "fill_variable_in_md_template".to_string(),
            fill_variable_in_md_template_desc.clone(),
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
            generator
                .generate_embedding_default(&fill_variable_in_md_template_desc)
                .await
                .unwrap(),
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

        let return_error_message_desc = "The error message to return. Useful for debugging in workflows.".to_string();
        tools.push(RustTool::new(
            "return_error_message".to_string(),
            return_error_message_desc.clone(),
            vec![ToolArgument::new(
                "error_message".to_string(),
                "string".to_string(),
                "The error message to return. Useful for debugging in workflows.".to_string(),
                true,
            )],
            generator
                .generate_embedding_default(&return_error_message_desc)
                .await
                .unwrap(),
        ));

        let count_files_from_input_desc = "Counts files with a specific extension.".to_string();
        tools.push(RustTool::new(
            "count_files_from_input".to_string(),
            count_files_from_input_desc.clone(),
            vec![ToolArgument::new(
                "file_extension".to_string(),
                "string".to_string(),
                "The file extension to count (optional)".to_string(),
                false,
            )],
            generator
                .generate_embedding_default(&count_files_from_input_desc)
                .await
                .unwrap(),
        ));

        let retrieve_file_from_input_desc = "Retrieves a file by name.".to_string();
        tools.push(RustTool::new(
            "retrieve_file_from_input".to_string(),
            retrieve_file_from_input_desc.clone(),
            vec![ToolArgument::new(
                "filename".to_string(),
                "string".to_string(),
                "The filename to retrieve".to_string(),
                true,
            )],
            generator
                .generate_embedding_default(&retrieve_file_from_input_desc)
                .await
                .unwrap(),
        ));

        let extract_and_map_csv_column_desc = "Extracts and maps a CSV column.".to_string();
        tools.push(RustTool::new(
            "extract_and_map_csv_column".to_string(),
            extract_and_map_csv_column_desc.clone(),
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
            generator
                .generate_embedding_default(&extract_and_map_csv_column_desc)
                .await
                .unwrap(),
        ));

        let process_embeddings_in_job_scope_desc = "Processes embeddings in job scope.".to_string();
        tools.push(RustTool::new(
            "process_embeddings_in_job_scope".to_string(),
            process_embeddings_in_job_scope_desc.clone(),
            vec![ToolArgument::new(
                "map_function".to_string(),
                "Box<dyn Fn(&str) -> String + Send + Sync>".to_string(),
                "The map function".to_string(),
                true,
            )],
            generator
                .generate_embedding_default(&process_embeddings_in_job_scope_desc)
                .await
                .unwrap(),
        ));

        tools
    }

    pub fn convert_args_from_fn_call(function_args: serde_json::Value) -> Result<Vec<Box<dyn Any + Send>>, LLMProviderError> {
        match function_args {
            serde_json::Value::Array(arr) => arr
                .into_iter()
                .map(|arg| match arg {
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
                    _ => Ok(Box::new(arg.to_string()) as Box<dyn Any + Send>),
                })
                .collect(),
            serde_json::Value::Object(map) => map
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
                .collect(),
            _ => Err(LLMProviderError::InvalidFunctionArguments(format!(
                "Invalid arguments: {:?}",
                function_args
            ))),
        }
    }
}
