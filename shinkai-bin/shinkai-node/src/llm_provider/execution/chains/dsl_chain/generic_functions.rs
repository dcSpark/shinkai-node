use csv::ReaderBuilder;
use futures::{future::join_all, StreamExt};
use html2md::parse_html;
use scraper::{Html, Selector};
use shinkai_dsl::sm_executor::WorkflowError;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::{any::Any, collections::HashMap};

use crate::llm_provider::{
    execution::{chains::inference_chain_trait::InferenceChainContextTrait, prompts::subprompts::SubPrompt},
    job_manager::JobManager,
};

use super::split_text_for_llm::split_text_for_llm;

// TODO: we need to generate description for each function (LLM processing?)
// we need to extend the description with keywords maybe use RAKE as well
// then we need to generate embeddings for them
// TODO: We need a file that contains the embeddings for the functions
// TODO: implement a new tool_router where we can instantiate it with embeddings and have handy fn for search and usage

pub struct RustToolFunctions;

impl RustToolFunctions {
    fn get_tool_map() -> HashMap<&'static str, RustToolFunction> {
        let mut tool_map: HashMap<&str, RustToolFunction> = HashMap::new();

        tool_map.insert("concat_strings", concat_strings);
        tool_map.insert("search_and_replace", search_and_replace);
        tool_map.insert("download_webpage", download_webpage);
        tool_map.insert("html_to_markdown", html_to_markdown);
        tool_map.insert("array_to_markdown_template", array_to_markdown_template);
        tool_map.insert("fill_variable_in_md_template", fill_variable_in_md_template);
        // tool_map.insert("print_arg", print_arg);
        tool_map.insert("return_error_message", return_error_message);
        tool_map.insert("count_files_from_input", count_files_from_input);
        tool_map.insert("retrieve_file_from_input", retrieve_file_from_input);
        tool_map.insert("extract_and_map_csv_column", extract_and_map_csv_column);

        tool_map.insert("process_embeddings_in_job_scope", process_embeddings_in_job_scope);
        tool_map.insert("split_text_for_llm", split_text_for_llm);

        tool_map
    }

    pub fn get_tool_function(name: &str) -> Option<RustToolFunction> {
        let tool_map = Self::get_tool_map();
        tool_map.get(name).copied()
    }
}

// Type alias for the function signature
type RustToolFunction =
    fn(&dyn InferenceChainContextTrait, Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError>;

// TODO: implement a new trait per Rust Tool

pub fn concat_strings(
    _context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() < 2 || args.len() > 4 {
        return Err(WorkflowError::InvalidArgument("Expected 2 to 4 arguments".to_string()));
    }

    let mut concatenated_string = String::new();

    for arg in args {
        let str = arg
            .downcast_ref::<String>()
            .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument".to_string()))?;
        concatenated_string.push_str(str);
    }

    Ok(Box::new(concatenated_string))
}

#[allow(dead_code)]
pub fn search_and_replace(
    _context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 3 {
        return Err(WorkflowError::InvalidArgument("Expected 3 arguments".to_string()));
    }
    let text = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for text".to_string()))?;
    let search = args[1]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for search".to_string()))?;
    let replace = args[2]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for replace".to_string()))?;

    Ok(Box::new(text.replace(search, replace)))
}

#[allow(dead_code)]
pub fn download_webpage(
    _context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 1 {
        return Err(WorkflowError::InvalidArgument("Expected 1 argument".to_string()));
    }
    let url = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for URL".to_string()))?
        .clone();

    let result = tokio::runtime::Runtime::new()
        .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?
        .block_on(async {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .redirect(reqwest::redirect::Policy::limited(20))
                .build()
                .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;
            let response = client
                .get(&url)
                .send()
                .await
                .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;
            let content = response
                .text()
                .await
                .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;
            Ok::<_, WorkflowError>(content)
        })?;

    Ok(Box::new(result))
}

#[allow(dead_code)]
pub fn html_to_markdown(
    _context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 1 {
        return Err(WorkflowError::InvalidArgument("Expected 1 argument".to_string()));
    }
    let html_content = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for HTML content".to_string()))?
        .clone();

    let document = Html::parse_document(&html_content);

    // Remove script and style elements
    let selector = Selector::parse("script, style").unwrap();
    let mut cleaned_html = document.root_element().inner_html();
    for element in document.select(&selector) {
        cleaned_html = cleaned_html.replace(&element.html(), "");
    }

    let markdown = parse_html(&cleaned_html);

    Ok(Box::new(markdown))
}

#[allow(dead_code)]
pub fn array_to_markdown_template(
    _context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 1 {
        return Err(WorkflowError::InvalidArgument("Expected 1 argument".to_string()));
    }
    let input = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for input string".to_string()))?
        .clone();

    let array: Vec<&str> = input.split(',').collect();
    let mut markdown = String::new();
    for item in array {
        markdown.push_str(&format!("## {}\n\n{{{{{}}}}}\n\n", item, item));
    }

    Ok(Box::new(markdown))
}

#[allow(dead_code)]
pub fn fill_variable_in_md_template(
    _context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 3 {
        return Err(WorkflowError::InvalidArgument("Expected 3 arguments".to_string()));
    }
    let template = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for template".to_string()))?
        .clone();
    let variable = args[1]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for variable".to_string()))?
        .clone();
    let content = args[2]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for content".to_string()))?
        .clone();

    let placeholder = format!("{{{{{}}}}}", variable);
    let filled_template = template.replace(&placeholder, &content);

    Ok(Box::new(filled_template))
}

#[allow(dead_code)]
pub fn print_arg(
    _context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 1 {
        return Err(WorkflowError::InvalidArgument("Expected 1 argument".to_string()));
    }
    let arg = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument".to_string()))?;

    println!("print_arg: {}", arg);
    Ok(Box::new(()))
}

#[allow(dead_code)]
pub fn return_error_message(
    _context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 1 {
        return Err(WorkflowError::InvalidArgument("Expected 1 argument".to_string()));
    }
    let error_message = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for error message".to_string()))?
        .clone();

    Err(WorkflowError::InvalidArgument(error_message))
}

#[allow(dead_code)]
pub fn count_files_from_input(
    context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    let extension = if args.is_empty() {
        None
    } else {
        let ext = args[0]
            .downcast_ref::<String>()
            .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for extension".to_string()))?;
        Some(ext.trim_start_matches('.').to_string())
    };

    let raw_files = context.raw_files();
    let count = match raw_files {
        Some(files) => files
            .iter()
            .filter(|(name, _)| {
                if let Some(ref ext) = extension {
                    name.ends_with(ext)
                } else {
                    true
                }
            })
            .count(),
        None => 0,
    };

    Ok(Box::new(count))
}

#[allow(dead_code)]
pub fn retrieve_file_from_input(
    context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 1 {
        return Err(WorkflowError::InvalidArgument("Expected 1 argument".to_string()));
    }
    let filename = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for filename".to_string()))?
        .clone();

    let raw_files = context.raw_files();
    if let Some(files) = raw_files {
        for (name, content) in files.iter() {
            if name == &filename {
                return Ok(Box::new(content.clone()));
            }
        }
    }

    Err(WorkflowError::InvalidArgument("File not found".to_string()))
}

#[allow(dead_code)]
pub fn extract_and_map_csv_column(
    _context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 3 {
        return Err(WorkflowError::InvalidArgument("Expected 3 arguments".to_string()));
    }

    let csv_data = args[0]
        .downcast_ref::<Vec<u8>>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for CSV data".to_string()))?
        .clone();
    let column_identifier = args[1]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for column identifier".to_string()))?
        .clone();
    let map_fn = args[2]
        .downcast_ref::<Box<dyn Fn(&str) -> String + Send>>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for map function".to_string()))?;

    let mut reader = ReaderBuilder::new().has_headers(true).from_reader(csv_data.as_slice());

    let headers = reader
        .headers()
        .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;
    let column_index = if let Ok(index) = column_identifier.parse::<usize>() {
        index
    } else {
        headers
            .iter()
            .position(|h| h == column_identifier)
            .ok_or_else(|| WorkflowError::InvalidArgument("Column not found".to_string()))?
    };

    let mut mapped_values = Vec::new();
    for result in reader.records() {
        let record = result.map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;
        if let Some(value) = record.get(column_index) {
            mapped_values.push(map_fn(value));
        }
    }

    Ok(Box::new(mapped_values))
}

pub fn process_embeddings_in_job_scope(
    context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() > 1 {
        return Err(WorkflowError::InvalidArgument("Expected 0 or 1 argument".to_string()));
    }

    let map_fn: &(dyn Fn(&str) -> String + Send + Sync) = if args.is_empty() {
        &|s: &str| s.to_string() // Default map function
    } else {
        args[0]
            .downcast_ref::<Box<dyn Fn(&str) -> String + Send + Sync>>()
            .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for map function".to_string()))?
            .as_ref()
    };

    // Be aware that although this function avoids starving other independently spawned tasks, any other code running concurrently in the same task will be
    // suspended during the call to block_in_place. This can happen e.g. when using the [join] macro. To avoid this issue, use [spawn_blocking] instead of block_in_place.
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Runtime::new()
            .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?
            .block_on(async {
                let vector_fs = context.vector_fs();
                let user_profile = context.user_profile();
                let scope = context.full_job().scope.clone();

                let resource_stream =
                    JobManager::retrieve_all_resources_in_job_scope_stream(vector_fs.clone(), &scope, user_profile)
                        .await;
                let mut chunks = resource_stream.chunks(5);

                let mut processed_embeddings = Vec::new();
                while let Some(resources) = chunks.next().await {
                    let futures = resources.into_iter().map(|resource| async move {
                        let subprompts = SubPrompt::convert_resource_into_subprompts_with_extra_info(&resource, 97);
                        let embedding = subprompts
                            .iter()
                            .map(|subprompt| map_fn(&subprompt.get_content()))
                            .collect::<Vec<String>>()
                            .join(" ");
                        Ok::<_, WorkflowError>(embedding)
                    });
                    let results = join_all(futures).await;

                    for result in results {
                        match result {
                            Ok(processed) => processed_embeddings.push(processed),
                            // TODO: change this to use another type of local printing
                            Err(e) => shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Error,
                                &format!("Error processing embedding: {}", e),
                            ),
                        }
                    }
                }

                let joined_results = processed_embeddings.join(":::");
                Ok::<_, WorkflowError>(joined_results)
            })
    })?;

    Ok(Box::new(result))
}

pub async fn search_embeddings_in_job_scope(
    context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 3 {
        return Err(WorkflowError::InvalidArgument(
            "Expected 3 arguments: query_text, num_of_top_results, max_tokens_in_prompt".to_string(),
        ));
    }

    let query_text = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for query_text".to_string()))?
        .clone();
    let num_of_top_results = *args[1]
        .downcast_ref::<u64>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for num_of_top_results".to_string()))?;
    let max_tokens_in_prompt = *args[2]
        .downcast_ref::<usize>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for max_tokens_in_prompt".to_string()))?;

    let db = context.db();
    let vector_fs = context.vector_fs();
    let user_profile = context.user_profile();
    let job_scope = context.full_job().scope.clone();
    let generator = context.generator();

    let result = JobManager::keyword_chained_job_scope_vector_search(
        db,
        vector_fs,
        &job_scope,
        query_text,
        user_profile,
        generator.clone(),
        num_of_top_results,
        max_tokens_in_prompt,
    )
    .await;

    match result {
        Ok((retrieved_nodes, _intro_text)) => {
            let formatted_results = retrieved_nodes
                .iter()
                .map(|node| node.node.get_text_content().unwrap_or_default().to_string())
                .collect::<Vec<String>>()
                .join(":::");
            Ok(Box::new(formatted_results))
        }
        Err(e) => {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Error,
                &format!("Error during vector search: {}", e),
            );
            Err(WorkflowError::ExecutionError(e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
        LLMProviderInterface, OpenAI, SerializedLLMProvider,
    };
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_message_primitives::shinkai_utils::job_scope::{JobScope, VectorFSFolderScopeEntry};
    use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use shinkai_vector_resources::source::VRSourceReference;
    use shinkai_vector_resources::vector_resource::{DocumentVectorResource, VectorResourceCore};
    use shinkai_vector_resources::{
        embedding_generator::RemoteEmbeddingGenerator,
        vector_resource::{BaseVectorResource, VRPath},
    };

    use crate::db::ShinkaiDB;
    use crate::llm_provider::execution::chains::dsl_chain::generic_functions::process_embeddings_in_job_scope;
    use crate::llm_provider::execution::chains::inference_chain_trait::InferenceChainContext;
    use crate::llm_provider::execution::{
        chains::{
            dsl_chain::generic_functions::{
                count_files_from_input, extract_and_map_csv_column, retrieve_file_from_input,
                search_embeddings_in_job_scope,
            },
            inference_chain_trait::MockInferenceChainContext,
        },
        user_message_parser::ParsedUserMessage,
    };
    use crate::llm_provider::job::Job;
    use crate::vector_fs::vector_fs::VectorFS;

    use super::{super::generic_functions::html_to_markdown, array_to_markdown_template, fill_variable_in_md_template};
    use std::{any::Any, collections::HashMap, fs, path::Path, sync::Arc};

    #[test]
    fn test_html_to_markdown() {
        let html_content = "<html><body><h1>Title</h1><p>This is a paragraph.</p><script>console.log('test');</script><style>body { font-size: 12px; }</style></body></html>";
        let args: Vec<Box<dyn Any + Send>> = vec![Box::new(html_content.to_string())];
        let context = MockInferenceChainContext::default();

        let result = html_to_markdown(&context, args);

        match result {
            Ok(markdown) => {
                let markdown_str = markdown.downcast_ref::<String>().unwrap();
                println!("Generated Markdown: {}", markdown_str);
                assert!(markdown_str.contains("Title"));
                assert!(markdown_str.contains("This is a paragraph."));
                assert!(!markdown_str.contains("console.log"));
                assert!(!markdown_str.contains("font-size"));
            }
            Err(e) => panic!("Test failed with error: {:?}", e),
        }
    }

    #[test]
    fn test_array_to_markdown() {
        let input = "red,blue,green".to_string();
        let args: Vec<Box<dyn Any + Send>> = vec![Box::new(input)];
        let context = MockInferenceChainContext::default();

        let result = array_to_markdown_template(&context, args);

        match result {
            Ok(markdown) => {
                let markdown_str = markdown.downcast_ref::<String>().unwrap();
                println!("Generated Markdown: {}", markdown_str);
                assert!(markdown_str.contains("## red\n\n{{red}}\n\n"));
                assert!(markdown_str.contains("## blue\n\n{{blue}}\n\n"));
                assert!(markdown_str.contains("## green\n\n{{green}}\n\n"));
            }
            Err(e) => panic!("Test failed with error: {:?}", e),
        }
    }

    #[test]
    fn test_fill_variable_in_md_template() {
        let template = "## red\n\n{{red}}\n\n## blue\n\n{{blue}}\n\n## green\n\n{{green}}\n\n".to_string();
        let variable = "red".to_string();
        let content = "the blood is red".to_string();
        let args: Vec<Box<dyn Any + Send>> = vec![Box::new(template), Box::new(variable), Box::new(content)];
        let context = MockInferenceChainContext::default();

        let result = fill_variable_in_md_template(&context, args);

        match result {
            Ok(filled_template) => {
                let filled_template_str = filled_template.downcast_ref::<String>().unwrap();
                println!("Filled Template: {}", filled_template_str);
                assert!(filled_template_str.contains("## red\n\nthe blood is red\n\n"));
                assert!(filled_template_str.contains("## blue\n\n{{blue}}\n\n"));
                assert!(filled_template_str.contains("## green\n\n{{green}}\n\n"));
            }
            Err(e) => panic!("Test failed with error: {:?}", e),
        }
    }

    #[test]
    fn test_count_files_from_input_no_extension() {
        let raw_files = Arc::new(vec![
            ("file1.txt".to_string(), vec![1, 2, 3]),
            ("file2.csv".to_string(), vec![4, 5, 6]),
            ("file3.txt".to_string(), vec![7, 8, 9]),
        ]);
        let context = MockInferenceChainContext::new(
            ParsedUserMessage {
                original_user_message_string: "".to_string(),
                elements: vec![],
            },
            HashMap::new(),
            ShinkaiName::default_testnet_localhost(),
            10,
            0,
            1000,
            HashMap::new(),
            Some(raw_files),
            None,
            None,
        );

        let args: Vec<Box<dyn Any + Send>> = vec![];
        let result = count_files_from_input(&context, args).unwrap();
        let count = result.downcast_ref::<usize>().unwrap();
        assert_eq!(*count, 3);
    }

    #[test]
    fn test_count_files_from_input_with_extension() {
        let raw_files = Arc::new(vec![
            ("file1.txt".to_string(), vec![1, 2, 3]),
            ("file2.csv".to_string(), vec![4, 5, 6]),
            ("file3.txt".to_string(), vec![7, 8, 9]),
        ]);
        let context = MockInferenceChainContext::new(
            ParsedUserMessage {
                original_user_message_string: "".to_string(),
                elements: vec![],
            },
            HashMap::new(),
            ShinkaiName::default_testnet_localhost(),
            10,
            0,
            1000,
            HashMap::new(),
            Some(raw_files),
            None,
            None,
        );

        let args: Vec<Box<dyn Any + Send>> = vec![Box::new("txt".to_string())];
        let result = count_files_from_input(&context, args).unwrap();
        let count = result.downcast_ref::<usize>().unwrap();
        assert_eq!(*count, 2);
    }

    #[test]
    fn test_retrieve_file_from_input() {
        let raw_files = Arc::new(vec![
            ("file1.txt".to_string(), vec![1, 2, 3]),
            ("file2.csv".to_string(), vec![4, 5, 6]),
        ]);
        let context = MockInferenceChainContext::new(
            ParsedUserMessage {
                original_user_message_string: "".to_string(),
                elements: vec![],
            },
            HashMap::new(),
            ShinkaiName::default_testnet_localhost(),
            10,
            0,
            1000,
            HashMap::new(),
            Some(raw_files),
            None,
            None,
        );

        let args: Vec<Box<dyn Any + Send>> = vec![Box::new("file2.csv".to_string())];
        let result = retrieve_file_from_input(&context, args).unwrap();
        let content = result.downcast_ref::<Vec<u8>>().unwrap();
        assert_eq!(content, &vec![4, 5, 6]);
    }

    #[test]
    fn test_retrieve_file_from_input_not_found() {
        let raw_files = Arc::new(vec![
            ("file1.txt".to_string(), vec![1, 2, 3]),
            ("file2.csv".to_string(), vec![4, 5, 6]),
        ]);
        let context = MockInferenceChainContext::new(
            ParsedUserMessage {
                original_user_message_string: "".to_string(),
                elements: vec![],
            },
            HashMap::new(),
            ShinkaiName::default_testnet_localhost(),
            10,
            0,
            1000,
            HashMap::new(),
            Some(raw_files),
            None,
            None,
        );

        let args: Vec<Box<dyn Any + Send>> = vec![Box::new("file3.txt".to_string())];
        let result = retrieve_file_from_input(&context, args);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_and_map_csv_column_by_header() {
        let csv_data = b"Name,Age,Location\nAlice,30,USA\nBob,25,UK\nCharlie,35,Canada".to_vec();
        let column_identifier = "Age".to_string();
        let map_fn: Box<dyn Fn(&str) -> String + Send> = Box::new(|value| format!("Age: {}", value));

        let args: Vec<Box<dyn Any + Send>> = vec![Box::new(csv_data), Box::new(column_identifier), Box::new(map_fn)];
        let context = MockInferenceChainContext::default();

        let result = extract_and_map_csv_column(&context, args).unwrap();
        let mapped_values = result.downcast_ref::<Vec<String>>().unwrap();
        assert_eq!(
            mapped_values,
            &vec!["Age: 30".to_string(), "Age: 25".to_string(), "Age: 35".to_string()]
        );
    }

    #[test]
    fn test_extract_and_map_csv_column_by_index() {
        let csv_data = b"Name,Age,Location\nAlice,30,USA\nBob,25,UK\nCharlie,35,Canada".to_vec();
        let column_identifier = "2".to_string(); // Location column
        let map_fn: Box<dyn Fn(&str) -> String + Send> = Box::new(|value| format!("Location: {}", value));

        let args: Vec<Box<dyn Any + Send>> = vec![Box::new(csv_data), Box::new(column_identifier), Box::new(map_fn)];
        let context = MockInferenceChainContext::default();

        let result = extract_and_map_csv_column(&context, args).unwrap();
        let mapped_values = result.downcast_ref::<Vec<String>>().unwrap();
        assert_eq!(
            mapped_values,
            &vec![
                "Location: USA".to_string(),
                "Location: UK".to_string(),
                "Location: Canada".to_string()
            ]
        );
    }

    #[test]
    fn test_extract_and_map_csv_column_invalid_column() {
        let csv_data = b"Name,Age,Location\nAlice,30,USA\nBob,25,UK\nCharlie,35,Canada".to_vec();
        let column_identifier = "InvalidColumn".to_string();
        let map_fn: Box<dyn Fn(&str) -> String + Send> = Box::new(|value| format!("Value: {}", value));

        let args: Vec<Box<dyn Any + Send>> = vec![Box::new(csv_data), Box::new(column_identifier), Box::new(map_fn)];
        let context = MockInferenceChainContext::default();

        let result = extract_and_map_csv_column(&context, args);
        assert!(result.is_err());
    }

    fn setup() {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(path);
    }

    fn default_test_profile() -> ShinkaiName {
        ShinkaiName::new("@@localhost.arb-sep-shinkai/main".to_string()).unwrap()
    }

    fn node_name() -> ShinkaiName {
        ShinkaiName::new("@@localhost.arb-sep-shinkai".to_string()).unwrap()
    }

    async fn setup_default_vector_fs() -> VectorFS {
        let generator = RemoteEmbeddingGenerator::new_default();
        let fs_db_path = format!("db_tests/{}", "vector_fs");
        let profile_list = vec![default_test_profile()];
        let supported_embedding_models = vec![EmbeddingModelType::OllamaTextEmbeddingsInference(
            OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M,
        )];

        VectorFS::new(
            generator,
            supported_embedding_models,
            profile_list,
            &fs_db_path,
            node_name(),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn test_search_embeddings_in_job_scope() {
        setup();
        let generator = RemoteEmbeddingGenerator::new_default();
        let vector_fs = setup_default_vector_fs().await;

        // Initialize ShinkaiDB
        let db_path = format!("db_tests/{}", "test_search_embeddings_in_job_scope");
        let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
        let shinkai_db_arc = Arc::new(shinkai_db);

        // Create a new job
        let job_id = "test_job_id".to_string();
        let agent_id = "test_agent_id".to_string();
        let job_scope = JobScope {
            local_vrkai: Vec::new(),
            local_vrpack: Vec::new(),
            vector_fs_items: Vec::new(),
            vector_fs_folders: vec![VectorFSFolderScopeEntry {
                path: VRPath::root(),
                name: "/".to_string(),
            }],
            network_folders: Vec::new(),
        };
        shinkai_db_arc
            .create_new_job(job_id.clone(), agent_id.clone(), job_scope.clone(), false)
            .unwrap();

        // Retrieve the created job
        let job = shinkai_db_arc.get_job(&job_id).unwrap();

        // Create a new folder and add a document to it
        let folder_name = "test_folder";
        let folder_path = VRPath::root().push_cloned(folder_name.to_string());
        let writer = vector_fs
            .new_writer(default_test_profile(), VRPath::root(), default_test_profile())
            .await
            .unwrap();
        vector_fs.create_new_folder(&writer, folder_name).await.unwrap();

        // Manually create documents for different topics
        let topics = vec![
            ("animals", "Animals are multicellular, eukaryotic organisms in the biological kingdom Animalia."),
            ("airplanes", "An airplane is a powered, fixed-wing aircraft that is propelled forward by thrust from a jet engine or propeller."),
            ("plants", "Plants are mainly multicellular organisms, predominantly photosynthetic eukaryotes of the kingdom Plantae."),
            ("cars", "A car is a wheeled motor vehicle used for transportation."),
            ("dinosaurs", "Dinosaurs are a diverse group of reptiles of the clade Dinosauria.")
        ];

        for (name, content) in topics {
            let mut doc = DocumentVectorResource::new_empty(
                name,
                Some(content),
                VRSourceReference::new_uri_ref("example.com"),
                true,
            );
            doc.set_embedding_model_used(generator.model_type());
            doc.keywords_mut().set_keywords(vec![name.to_string()]);
            doc.update_resource_embedding(&generator, None).await.unwrap();
            let content_embedding = generator.generate_embedding_default(content).await.unwrap();
            doc.append_text_node(content, None, content_embedding, &vec![]).unwrap();

            let writer = vector_fs
                .new_writer(default_test_profile(), folder_path.clone(), default_test_profile())
                .await
                .unwrap();
            vector_fs
                .save_vector_resource_in_folder(&writer, BaseVectorResource::Document(doc), None)
                .await
                .unwrap();
        }

        // Create a SerializedLLMProvider instance
        let open_ai = OpenAI {
            model_type: "gpt-4-1106-preview".to_string(),
        };

        let agent_name = ShinkaiName::new("@@localhost.arb-sep-shinkai/main/agent/testAgent".to_string()).unwrap();
        let agent = SerializedLLMProvider {
            id: "test_agent_id".to_string(),
            full_identity_name: agent_name,
            perform_locally: false,
            external_url: Some("https://api.openai.com".to_string()),
            api_key: Some("mockapikey".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
            toolkit_permissions: vec![],
            storage_bucket_permissions: vec![],
            allowed_message_senders: vec![],
        };

        // Create a full InferenceChainContext with the generated embeddings
        let context = InferenceChainContext::new(
            shinkai_db_arc,
            Arc::new(vector_fs),
            job,
            ParsedUserMessage {
                original_user_message_string: "".to_string(),
                elements: vec![],
            },
            agent,
            HashMap::new(),
            generator,
            ShinkaiName::default_testnet_localhost(),
            10,
            1000,
            HashMap::new(),
            None, // Replace with actual WSUpdateHandler if needed
            None, // Replace with actual ToolRouter if needed
        );

        // Call the function to process embeddings in job scope
        let args: Vec<Box<dyn Any + Send>> = vec![];
        let result = tokio::task::spawn_blocking(move || process_embeddings_in_job_scope(&context, args))
            .await
            .unwrap()
            .unwrap();

        // Validate the processed embeddings
        let processed_embeddings = result.downcast_ref::<String>().unwrap();
        eprintln!("Processed embeddings: {}", processed_embeddings);
        assert!(!processed_embeddings.is_empty());
        assert!(processed_embeddings.contains("Animals are multicellular, eukaryotic organisms"));
        assert!(processed_embeddings.contains("An airplane is a powered, fixed-wing aircraft"));
        assert!(processed_embeddings.contains("Plants are mainly multicellular organisms"));
        assert!(processed_embeddings.contains("A car is a wheeled motor vehicle"));
        assert!(processed_embeddings.contains("Dinosaurs are a diverse group of reptiles"));
    }

    #[tokio::test]
    async fn test_search_embeddings_in_job_scope_with_query() {
        setup();
        let generator = RemoteEmbeddingGenerator::new_default();
        let vector_fs = setup_default_vector_fs().await;

        // Initialize ShinkaiDB
        let db_path = format!("db_tests/{}", "test_search_embeddings_in_job_scope_with_query");
        let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
        let shinkai_db_arc = Arc::new(shinkai_db);

        // Create a new job
        let job_id = "test_job_id_with_query".to_string();
        let agent_id = "test_agent_id_with_query".to_string();
        let job_scope = JobScope {
            local_vrkai: Vec::new(),
            local_vrpack: Vec::new(),
            vector_fs_items: Vec::new(),
            vector_fs_folders: vec![VectorFSFolderScopeEntry {
                path: VRPath::root(),
                name: "/".to_string(),
            }],
            network_folders: Vec::new(),
        };
        shinkai_db_arc
            .create_new_job(job_id.clone(), agent_id.clone(), job_scope.clone(), false)
            .unwrap();

        // Retrieve the created job
        let job = shinkai_db_arc.get_job(&job_id).unwrap();

        // Create a new folder and add a document to it
        let folder_name = "test_folder_with_query";
        let folder_path = VRPath::root().push_cloned(folder_name.to_string());
        let writer = vector_fs
            .new_writer(default_test_profile(), VRPath::root(), default_test_profile())
            .await
            .unwrap();
        vector_fs.create_new_folder(&writer, folder_name).await.unwrap();

        // Manually create documents for different topics
        let topics = vec![
        ("animals", "Animals are multicellular, eukaryotic organisms in the biological kingdom Animalia."),
        ("airplanes", "An airplane is a powered, fixed-wing aircraft that is propelled forward by thrust from a jet engine or propeller."),
        ("plants", "Plants are mainly multicellular organisms, predominantly photosynthetic eukaryotes of the kingdom Plantae."),
        ("cars", "A car is a wheeled motor vehicle used for transportation."),
        ("dinosaurs", "Dinosaurs are a diverse group of reptiles of the clade Dinosauria.")
    ];

        for (name, content) in topics {
            let mut doc = DocumentVectorResource::new_empty(
                name,
                Some(content),
                VRSourceReference::new_uri_ref("example.com"),
                true,
            );
            doc.set_embedding_model_used(generator.model_type());
            doc.keywords_mut().set_keywords(vec![name.to_string()]);
            doc.update_resource_embedding(&generator, None).await.unwrap();
            let content_embedding = generator.generate_embedding_default(content).await.unwrap();
            doc.append_text_node(content, None, content_embedding, &vec![]).unwrap();

            let writer = vector_fs
                .new_writer(default_test_profile(), folder_path.clone(), default_test_profile())
                .await
                .unwrap();
            vector_fs
                .save_vector_resource_in_folder(&writer, BaseVectorResource::Document(doc), None)
                .await
                .unwrap();
        }

        // Create a SerializedLLMProvider instance
        let open_ai = OpenAI {
            model_type: "gpt-4-1106-preview".to_string(),
        };

        let agent_name =
            ShinkaiName::new("@@localhost.arb-sep-shinkai/main/agent/testAgentWithQuery".to_string()).unwrap();
        let agent = SerializedLLMProvider {
            id: "test_agent_id_with_query".to_string(),
            full_identity_name: agent_name,
            perform_locally: false,
            external_url: Some("https://api.openai.com".to_string()),
            api_key: Some("mockapikey".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
            toolkit_permissions: vec![],
            storage_bucket_permissions: vec![],
            allowed_message_senders: vec![],
        };

        // Create a full InferenceChainContext with the generated embeddings
        let context = InferenceChainContext::new(
            shinkai_db_arc,
            Arc::new(vector_fs),
            job,
            ParsedUserMessage {
                original_user_message_string: "".to_string(),
                elements: vec![],
            },
            agent,
            HashMap::new(),
            generator,
            ShinkaiName::default_testnet_localhost(),
            10,
            1000,
            HashMap::new(),
            None, // Replace with actual WSUpdateHandler if needed
            None, // Replace with actual ToolRouter if needed
        );

        // Call the function to search embeddings in job scope
        let query_text = "What are multicellular organisms?".to_string();
        let num_of_top_results: u64 = 3;
        let max_tokens_in_prompt: usize = 100;
        let args: Vec<Box<dyn Any + Send>> = vec![
            Box::new(query_text),
            Box::new(num_of_top_results),
            Box::new(max_tokens_in_prompt),
        ];
        let result = search_embeddings_in_job_scope(&context, args).await.unwrap();

        // Validate the search results
        let search_results = result.downcast_ref::<String>().unwrap();
        eprintln!("Search results: {}", search_results);
        assert!(!search_results.is_empty());

        // Check that the top results are in the expected order
        let results: Vec<&str> = search_results.split(":::").collect();
        assert_eq!(
            results[0],
            "Animals are multicellular, eukaryotic organisms in the biological kingdom Animalia."
        );
        assert_eq!(results[1], "Plants are mainly multicellular organisms, predominantly photosynthetic eukaryotes of the kingdom Plantae.");
        assert_eq!(
            results[2],
            "Dinosaurs are a diverse group of reptiles of the clade Dinosauria."
        );
    }
}
