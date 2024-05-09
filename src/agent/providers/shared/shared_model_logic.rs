use regex::Regex;
use serde_json;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::{
    schemas::agents::serialized_agent::AgentLLMInterface,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};

use crate::{
    agent::{
        error::AgentError,
        execution::prompts::prompts::{Prompt, SubPrompt},
    },
    managers::model_capabilities_manager::{
        Base64ImageString, ModelCapabilitiesManager, PromptResult, PromptResultEnum,
    },
};

pub fn llama_prepare_messages(
    _model: &AgentLLMInterface,
    _model_type: String,
    prompt: Prompt,
    total_tokens: usize,
) -> Result<PromptResult, AgentError> {
    let messages_string = prompt.generate_genericapi_messages(Some(total_tokens))?;

    let used_tokens = ModelCapabilitiesManager::count_tokens_from_message_llama3(&messages_string);

    Ok(PromptResult {
        value: PromptResultEnum::Text(messages_string.clone()),
        remaining_tokens: total_tokens - used_tokens,
    })
}

pub fn llava_prepare_messages(
    _model: &AgentLLMInterface,
    _model_type: String,
    prompt: Prompt,
    total_tokens: usize,
) -> Result<PromptResult, AgentError> {
    let messages_string = prompt.generate_genericapi_messages(Some(total_tokens))?;

    if let Some((_, _, asset_content, _, _)) = prompt.sub_prompts.iter().rev().find_map(|sub_prompt| {
        if let SubPrompt::Asset(prompt_type, asset_type, asset_content, asset_detail, priority) = sub_prompt {
            Some((prompt_type, asset_type, asset_content, asset_detail, priority))
        } else {
            None
        }
    }) {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            format!("Messages JSON (image analysis): {:?}", messages_string).as_str(),
        );

        Ok(PromptResult {
            value: PromptResultEnum::ImageAnalysis(messages_string.clone(), Base64ImageString(asset_content.clone())),
            remaining_tokens: total_tokens - messages_string.len(),
        })
    } else {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Error,
            format!("Image content not found: {:?}", messages_string).as_str(),
        );
        Err(AgentError::ImageContentNotFound("Image content not found".to_string()))
    }
}

pub fn parse_markdown_to_json(markdown: &str) -> Result<JsonValue, AgentError> {
    // Find the index of the first '#' and slice the string from there
    let start_index = markdown.find('#').unwrap_or(0);
    let trimmed_markdown = &markdown[start_index..];

    let mut sections = serde_json::Map::new();
    // let re = Regex::new(r"(?m)^# (\w+)$").unwrap();
    let re = Regex::new(r"(?m)^#\s+(.+)$").unwrap();
    let mut current_section = None;
    let mut content = String::new();

    for line in trimmed_markdown.lines() {
        if let Some(caps) = re.captures(line) {
            if let Some(section) = current_section {
                sections.insert(section, JsonValue::String(content.trim().to_string()));
                content.clear();
            }
            current_section = Some(caps[1].to_string());
        } else if current_section.is_some() {
            content.push_str(line);
            content.push('\n');
        } else {
            current_section = Some("".to_string());
            content.push_str(line);
            content.push('\n');
        }
    }

    if let Some(section) = current_section {
        sections.insert(section, JsonValue::String(content.trim().to_string()));
    }

    Ok(JsonValue::Object(sections))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_to_json_single_section() {
        let markdown = "# Section1\nContent line 1\nContent line 2";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({
            "Section1": "Content line 1\nContent line 2"
        });
        assert_eq!(result, expected_json);
    }

    #[test]
    fn test_parse_markdown_to_json_multiple_sections() {
        let markdown = "# Section1\nContent line 1\n# Section2\nContent line 2";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({
            "Section1": "Content line 1",
            "Section2": "Content line 2"
        });
        assert_eq!(result, expected_json);
    }

    #[test]
    fn test_parse_markdown_to_json_no_sections() {
        let markdown = "No section content here";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({
            "": "No section content here"
        });
        assert_eq!(result, expected_json);
    }

    #[test]
    fn test_parse_markdown_to_json_empty_input() {
        let markdown = "";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({});
        assert_eq!(result, expected_json);
    }

    #[test]
    fn test_parse_markdown_to_json_complex_content() {
        let markdown = "# Section1\nContent line 1\nContent line 2\n# Section2\nContent line 3\nContent line 4";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({
            "Section1": "Content line 1\nContent line 2",
            "Section2": "Content line 3\nContent line 4"
        });
        assert_eq!(result, expected_json);
    }

    #[test]
    fn test_parse_markdown_to_json_with_given_content() {
        let markdown = "\n# Answer\n Hello there, how may I assist you today?\n# Summary\n Answer's summary";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({
            "Answer": "Hello there, how may I assist you today?",
            "Summary": "Answer's summary"
        });
        assert_eq!(result, expected_json);
    }
}