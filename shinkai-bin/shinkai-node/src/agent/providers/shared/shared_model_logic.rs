use regex::Regex;
use serde_json;
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
    let mut markdown = markdown;

    // Check if the text starts with "```markdown" and remove it along with the ending triple quotes
    if markdown.starts_with("```markdown") {
        markdown = markdown.trim_start_matches("```markdown").trim();
        if markdown.ends_with("```") {
            markdown = markdown.trim_end_matches("```").trim();
        }
    }

    // Find the index of the first '#' and slice the string from there
    let start_index = markdown.find('#').unwrap_or(0);
    let trimmed_markdown = &markdown[start_index..];

    let mut sections = serde_json::Map::new();
    // let re = Regex::new(r"(?m)^# (\w+)$").unwrap();
    let re = Regex::new(r"(?m)^#\s+(.+)$").unwrap();
    let mut current_section: Option<String> = None;
    let mut content = String::new();

    for line in trimmed_markdown.lines() {
        if let Some(caps) = re.captures(line) {
            if let Some(section) = current_section {
                sections.insert(section.trim().to_lowercase(), JsonValue::String(content.trim().to_string()));
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
        if !sections.contains_key(&section.to_lowercase()) {
            sections.insert(
                section.trim().to_lowercase().to_string(),
                JsonValue::String(content.trim().to_string()),
            );
        }
    }

    Ok(JsonValue::Object(sections))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_parse_markdown_to_json_single_section() {
        let markdown = "# Section1\nContent line 1\nContent line 2";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({
            "section1": "Content line 1\nContent line 2"
        });
        assert_eq!(result, expected_json);
    }

    #[test]
    fn test_parse_markdown_to_json_multiple_sections() {
        let markdown = "# Section1\nContent line 1\n# Section2\nContent line 2";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({
            "section1": "Content line 1",
            "section2": "Content line 2"
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
            "section1": "Content line 1\nContent line 2",
            "section2": "Content line 3\nContent line 4"
        });
        assert_eq!(result, expected_json);
    }

    #[test]
    fn test_parse_markdown_to_json_with_given_content() {
        let markdown = "\n# Answer \n Hello there, how may I assist you today?\n# Summary\n Answer's summary";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({
            "answer": "Hello there, how may I assist you today?",
            "summary": "Answer's summary"
        });
        assert_eq!(result, expected_json);
    }

    #[test]
    fn test_parse_markdown_to_json_answer_section() {
        let markdown = "# Answer\nYes, I am here. How can I assist you today?";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({
            "answer": "Yes, I am here. How can I assist you today?"
        });
        assert_eq!(result, expected_json);
    }

    #[test]
    fn test_parse_markdown_to_json_with_special_characters() {
        let markdown = "```markdown\n# Answer\n\nThe Roman Empire was one of the largest and most influential...\n```";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({
            "answer": "The Roman Empire was one of the largest and most influential..."
        });
        assert_eq!(result, expected_json);
    }

    #[test]
    fn test_parse_markdown_to_json_multiple_answers() {
        let markdown = "# Answer\nThe zodiac has no special significance in the modern world, but it was an important concept in ancient times. For example, if we were to create a joke about the zodiac, it could be as follows: \"Why did the fish cross the zodiac?\" The punchline would be that the fish is asking why he had to go through all of the challenges and obstacles required to travel across the various parts of the sky.\n\n# Answer\nThe Roman Empire was a significant civilization with its impact on art, law, culture, technology, economics, religion, philosophy, and military tactics.";
        let result = parse_markdown_to_json(markdown).unwrap();
        let expected_json = json!({
            "answer": "The zodiac has no special significance in the modern world, but it was an important concept in ancient times. For example, if we were to create a joke about the zodiac, it could be as follows: \"Why did the fish cross the zodiac?\" The punchline would be that the fish is asking why he had to go through all of the challenges and obstacles required to travel across the various parts of the sky."
        });
        assert_eq!(result, expected_json);
    }
}
