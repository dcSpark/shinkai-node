use csv::ReaderBuilder;
use futures::{future::join_all, StreamExt};
use html2md::parse_html;
use scraper::{Html, Selector};
use shinkai_dsl::sm_executor::WorkflowError;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::any::Any;

use crate::llm_provider::{
    execution::{chains::inference_chain_trait::InferenceChainContextTrait, prompts::subprompts::SubPrompt}, job_manager::JobManager,
};

// TODO: we need to generate description for each function (LLM processing?)
// we need to extend the description with keywords maybe use RAKE as well
// then we need to generate embeddings for them
// TODO: We need a file that contains the embeddings for the functions
// TODO: implement a new tool_router where we can instantiate it with embeddings and have handy fn for search and usage

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

#[allow(dead_code)]
// TODO: needs some work in the embedding <> fn usage
pub async fn process_embeddings_in_job_scope(
    context: &dyn InferenceChainContextTrait,
    args: Vec<Box<dyn Any + Send>>,
) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 1 {
        return Err(WorkflowError::InvalidArgument("Expected 1 argument".to_string()));
    }

    let map_fn = args[0]
        .downcast_ref::<Box<dyn Fn(&str) -> String + Send + Sync>>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for map function".to_string()))?;

    let vector_fs = context.vector_fs();
    let user_profile = context.user_profile();
    let scope = context.full_job().scope.clone();

    let resource_stream =
        JobManager::retrieve_all_resources_in_job_scope_stream(vector_fs.clone(), &scope, user_profile).await;
    let mut chunks = resource_stream.chunks(5);

    let mut processed_embeddings = Vec::new();
    while let Some(resources) = chunks.next().await {
        let futures = resources.into_iter().map(|resource| async move {
            let subprompts = SubPrompt::convert_resource_into_subprompts(&resource, 97);
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
    Ok(Box::new(joined_results))
}
#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

    use crate::llm_provider::execution::{
        chains::{
            dsl_chain::generic_functions::{
                count_files_from_input, extract_and_map_csv_column, retrieve_file_from_input,
            },
            inference_chain_trait::MockInferenceChainContext,
        },
        user_message_parser::ParsedUserMessage,
    };

    use super::{super::generic_functions::html_to_markdown, array_to_markdown_template, fill_variable_in_md_template};
    use std::{any::Any, collections::HashMap, sync::Arc};

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
}
