use html2md::parse_html;
use scraper::{Html, Selector};
use shinkai_dsl::sm_executor::WorkflowError;
use std::any::Any;

pub fn concat_strings(args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 2 {
        return Err(WorkflowError::InvalidArgument("Expected 2 arguments".to_string()));
    }
    let str1 = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument".to_string()))?;
    let str2 = args[1]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument".to_string()))?;
    Ok(Box::new(format!("{}{}", str1, str2)))
}

pub fn search_and_replace(args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
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

pub fn download_webpage(args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
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

pub fn html_to_markdown(args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
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

pub fn array_to_markdown_template(args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
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

pub fn fill_variable_in_md_template(args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
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

pub fn print_arg(args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
    if args.len() != 1 {
        return Err(WorkflowError::InvalidArgument("Expected 1 argument".to_string()));
    }
    let arg = args[0]
        .downcast_ref::<String>()
        .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument".to_string()))?;
    
    println!("print_arg: {}", arg);
    Ok(Box::new(()))
}

#[cfg(test)]
mod tests {
    use super::{super::generic_functions::html_to_markdown, array_to_markdown_template, fill_variable_in_md_template};
    use std::any::Any;

    #[test]
    fn test_html_to_markdown() {
        let html_content = "<html><body><h1>Title</h1><p>This is a paragraph.</p><script>console.log('test');</script><style>body { font-size: 12px; }</style></body></html>";
        let args: Vec<Box<dyn Any + Send>> = vec![Box::new(html_content.to_string())];

        let result = html_to_markdown(args);

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

        let result = array_to_markdown_template(args);

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

        let result = fill_variable_in_md_template(args);

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
}
