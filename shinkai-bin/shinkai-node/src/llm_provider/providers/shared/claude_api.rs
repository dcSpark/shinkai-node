use crate::llm_provider::error::LLMProviderError;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::model_capabilities_manager::PromptResult;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use serde_json::{self};
use shinkai_message_primitives::schemas::llm_message::LlmMessage;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use shinkai_message_primitives::schemas::prompts::Prompt;

fn sanitize_tool_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();

    // Ensure length is between 1 and 64 characters
    if sanitized.is_empty() {
        "tool".to_string()
    } else {
        sanitized.chars().take(64).collect()
    }
}

pub fn claude_prepare_messages(
    model: &LLMProviderInterface,
    prompt: Prompt,
) -> Result<(PromptResult, Vec<LlmMessage>), LLMProviderError> {
    let max_input_tokens = ModelCapabilitiesManager::get_max_input_tokens(model);

    let chat_completion_messages = prompt.generate_llm_messages(
        Some(max_input_tokens),
        Some("function".to_string()),
        &ModelCapabilitiesManager::num_tokens_from_llama3,
    )?;

    process_llm_messages(chat_completion_messages, model)
}

pub fn process_llm_messages(
    chat_completion_messages: Vec<LlmMessage>,
    model: &LLMProviderInterface,
) -> Result<(PromptResult, Vec<LlmMessage>), LLMProviderError> {
    let used_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&chat_completion_messages);
    let remaining_output_tokens = ModelCapabilitiesManager::get_remaining_output_tokens(model, used_tokens);

    let (mut messages_with_role, tools): (Vec<_>, Vec<_>) = chat_completion_messages
        .into_iter()
        .partition(|message| message.role.is_some());

    let mut system_messages = Vec::new();

    for message in &mut messages_with_role {
        if message.role == Some("system".to_string()) {
            system_messages.push(message.clone());
        }
    }

    messages_with_role.retain(|message| {
        ((message.role == Some("user".to_string())
            || message.role == Some("function".to_string())
            || message.role == Some("assistant".to_string()))
            && message.content.is_some()
            && message.content.as_ref().unwrap().len() > 0)
            || (message.role == Some("assistant".to_string()) && message.function_call.is_some())
    });

    let messages_json = serde_json::to_value(messages_with_role)?;
    let tools_json = serde_json::to_value(tools)?;

    let messages_vec = match messages_json {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .map(|mut message| {
                if message.get("role") == Some(&serde_json::Value::String("user".to_string())) {
                    if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
                        message["content"] = serde_json::Value::String(content.to_string());
                    }
                    message.as_object_mut().unwrap().remove("images");
                }

                if message.get("role") == Some(&serde_json::Value::String("assistant".to_string())) {
                    if let Some(function_call) = message.get("function_call").cloned() {
                        if let Some(arguments) = function_call.get("arguments").and_then(|v| v.as_str()) {
                            let input: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();
                            let tool_name = function_call
                                .get("name")
                                .and_then(|n| n.as_str())
                                .map(sanitize_tool_name)
                                .unwrap_or_else(|| "tool".to_string());

                            message["content"] = serde_json::json!([
                                {
                                    "type": "tool_use",
                                    "id": "toolu_abc123",
                                    "name": tool_name,
                                    "input": input
                                }
                            ]);
                        }
                        message.as_object_mut().unwrap().remove("function_call");
                    } else if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
                        message["content"] = serde_json::Value::String(content.to_string());
                        message.as_object_mut().unwrap().remove("images");
                    }
                }

                if message.get("role") == Some(&serde_json::Value::String("function".to_string())) {
                    if let Some(content) = message.get("content").cloned() {
                        message["content"] = serde_json::json!([{
                            "type": "tool_result",
                            "tool_use_id": "toolu_abc123",
                            "content": content
                        }]);
                    }
                    message["role"] = serde_json::Value::String("user".to_string());
                    message.as_object_mut().unwrap().remove("name");
                }

                message
            })
            .collect(),
        _ => vec![],
    };

    let tools_vec = match tools_json {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .flat_map(|tool| {
                if let serde_json::Value::Object(mut map) = tool {
                    map.remove("functions")
                        .and_then(|functions| {
                            if let serde_json::Value::Array(funcs) = functions {
                                Some(
                                    funcs
                                        .into_iter()
                                        .map(|mut func| {
                                            if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                                                func.as_object_mut().unwrap()["name"] =
                                                    serde_json::Value::String(sanitize_tool_name(name));
                                            }
                                            func
                                        })
                                        .collect::<Vec<_>>(),
                                )
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default()
                } else {
                    vec![]
                }
            })
            .collect(),
        _ => vec![],
    };

    Ok((
        PromptResult {
            messages: PromptResultEnum::Value(serde_json::Value::Array(messages_vec)),
            functions: Some(tools_vec),
            remaining_output_tokens,
            tokens_used: used_tokens,
        },
        system_messages,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use serde_json::json;
    use shinkai_message_primitives::schemas::{
        llm_message::DetailedFunctionCall, llm_providers::serialized_llm_provider::{Claude, SerializedLLMProvider}, subprompts::{SubPrompt, SubPromptType}
    };

    #[test]
    fn test_claude_prepare_messages() {
        let claude_model = Claude {
            model_type: "claude-3-5-sonnet-20241022".to_string(),
        };

        let model = LLMProviderInterface::Claude(claude_model);

        let llm_messages = vec![
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("You are a very helpful assistant. You may be provided with documents or content to analyze and answer questions about them, in that case refer to the content provided in the user message for your responses.".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
                tool_calls: None,
            },
            LlmMessage {
                role: Some("user".to_string()),
                content: Some("tell me what's the response when using shinkai echo tool with: say hello".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: Some(vec![]),
                tool_calls: None,
            },
            LlmMessage {
                role: Some("assistant".to_string()),
                content: None,
                name: None,
                function_call: Some(DetailedFunctionCall {
                    name: "shinkai__echo".to_string(),
                    arguments: "{\"message\":\"hello\"}".to_string(),
                    id: None,
                }),
                functions: None,
                images: None,
                tool_calls: None,
            },
            LlmMessage {
                role: Some("function".to_string()),
                content: Some("{\"data\":{\"message\":\"echoing: hello\"}}".to_string()),
                name: Some("shinkai__echo".to_string()),
                function_call: None,
                functions: None,
                images: None,
                tool_calls: None,
            },
        ];

        let expected_json = json!([
          {
            "role": "user",
            "content": "tell me what's the response when using shinkai echo tool with: say hello"
          },
          {
            "role": "assistant",
            "content": [
          {
            "type": "tool_use",
            "id": "toolu_abc123",
            "name": "shinkai__echo",
            "input": {
              "message": "hello"
            }
          }
            ]
          },
          {
            "role": "user",
            "content": [
              {
                "type": "tool_result",
                "tool_use_id": "toolu_abc123",
                "content": "{\"data\":{\"message\":\"echoing: hello\"}}"
              }
            ]
          }
        ]);

        let (messages_result, _system_messages) = process_llm_messages(llm_messages, &model).unwrap();
        let messages_json = match messages_result.messages {
            PromptResultEnum::Value(v) => v,
            _ => {
                panic!("Expected Value variant in PromptResultEnum");
            }
        };
        assert_eq!(messages_json, expected_json);
    }

    #[test]
    fn test_tool_name_sanitization() {
        let sub_prompts = vec![
            SubPrompt::Content(
                SubPromptType::System,
                "You are a very helpful assistant. You may be provided with documents or content to analyze and answer questions about them, in that case refer to the content provided in the user message for your responses.".to_string(),
                98,
            ),
            SubPrompt::ToolAvailable(
                SubPromptType::AvailableTool,
                serde_json::json!({
                    "function": {
                        "description": "Searches the DuckDuckGo search engine. Example result: [{\"title\": \"IMDb Top 250 Movies\", \"description\": \"Find out which <b>movies</b> are rated as the <b>best</b> <b>of</b> <b>all</b> <b>time</b> by IMDb users. See the list of 250 titles sorted by ranking, genre, year, and rating, and learn how the list is determined.\", \"url\": \"https://www.imdb.com/chart/top/\"}]",
                        "name": "DuckDuckGo Search",
                        "parameters": {
                            "properties": {
                                "message": {
                                    "description": "The search query to send to DuckDuckGo",
                                    "type": "string"
                                }
                            },
                            "required": ["message"],
                            "type": "object"
                        },
                        "tool_router_key": "local:::duckduckgo_search:::duckduckgo_search"
                    },
                    "type": "function"
                }),
                98,
            ),
            SubPrompt::Omni(
                SubPromptType::UserLastMessage,
                "duckduckgo search for movies".to_string(),
                vec![],
                100,
            ),
            SubPrompt::FunctionCall(
                SubPromptType::Assistant,
                serde_json::json!({
                    "arguments": {
                        "message": "movies"
                    },
                    "name": "duckduckgo_search"
                }),
                100,
            ),
            SubPrompt::FunctionCallResponse(
                SubPromptType::Function,
                serde_json::json!({
                    "function_call": {
                        "arguments": {
                            "message": "movies"
                        },
                        "name": "duckduckgo_search",
                        "response": null,
                        "tool_router_key": null
                    },
                    "response": "{\"data\":{\"__created_files__\":[\"shinkai://file/@@my_local_ai.sep-shinkai/main/jobid_c93837a6-358b-4648-9617-1d6e93d0bb59/logs/log_jobid_c93837a6-358b-4648-9617-1d6e93d0bb59_localduckduckgo_searchduckduckgo_search.log\"],\"message\":\"[{\\\"title\\\":\\\"Movie Tickets &amp; Movie Times | Fandango\\\",\\\"description\\\":\\\"Honoring the Best <b>movies</b> &amp; TV. Check out the winners from this year&#x27;s 26th annual Rotten Tomatoes Awards. LEARN MORE. Collectors, assemble! image link. Collectors, assemble! Suit up and get the Captain America Collector Pack, featuring an exclusive Collector&#x27;s Coin, Limited-Edition Poster, and one <b>movie</b> ticket!\\\",\\\"url\\\":\\\"https://www.fandango.com/\\\"}]\"}}",
                }),
                100,
            ),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        let model = SerializedLLMProvider::mock_provider().model;

        let (result, _) = claude_prepare_messages(&model, prompt).unwrap();
        let messages = match result.messages {
            PromptResultEnum::Value(v) => v,
            _ => panic!("Expected Value variant"),
        };

        eprintln!("messages: {:?}", messages);

        // Find the assistant message with tool use
        let tool_use_message = messages
            .as_array()
            .unwrap()
            .iter()
            .find(|msg| {
                msg.get("role") == Some(&json!("assistant"))
                    && msg
                        .get("content")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|obj| obj.get("type"))
                        == Some(&json!("tool_use"))
            })
            .expect("Should have found a tool use message");

        // Extract the tool name
        let tool_name = tool_use_message
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|obj| obj.get("name"))
            .and_then(|n| n.as_str())
            .expect("Should have a tool name");

        // Verify the tool name matches our pattern
        let name_pattern = Regex::new(r"^[a-zA-Z0-9_-]{1,64}$").unwrap();
        eprintln!("Tool name: {}", tool_name);
        assert!(
            name_pattern.is_match(tool_name),
            "Tool name '{}' should match pattern ^[a-zA-Z0-9_-]{{1,64}}$",
            tool_name
        );
        assert_eq!(
            tool_name, "duckduckgo_search",
            "Tool name should be sanitized correctly"
        );
    }

    #[test]
    fn test_claude_prepare_messages_with_multiple_tools() {
        let sub_prompts = vec![
            SubPrompt::Content(
                SubPromptType::System,
                "You are a very helpful assistant. You may be provided with documents or content to analyze and answer questions about them, in that case refer to the content provided in the user message for your responses.".to_string(),
                98,
            ),
            SubPrompt::ToolAvailable(
                SubPromptType::AvailableTool,
                serde_json::json!({
                    "function": {
                        "description": "This function takes a question as input and returns a comprehensive answer, along with the sources and statements used to generate the answer.",
                        "name": "Google Search",
                        "parameters": {
                            "properties": {
                                "query": {
                                    "description": "The search query to look up",
                                    "type": "string"
                                }
                            },
                            "required": ["query"],
                            "type": "object"
                        },
                        "tool_router_key": "local:::google_search_shinkai:::google_search"
                    },
                    "type": "function"
                }),
                98,
            ),
            SubPrompt::ToolAvailable(
                SubPromptType::AvailableTool,
                serde_json::json!({
                    "function": {
                        "description": "Searches the DuckDuckGo search engine. Example result: [{\"title\": \"IMDb Top 250 Movies\", \"description\": \"Find out which <b>movies</b> are rated as the <b>best</b> <b>of</b> <b>all</b> <b>time</b> by IMDb users. See the list of 250 titles sorted by ranking, genre, year, and rating, and learn how the list is determined.\", \"url\": \"https://www.imdb.com/chart/top/\"}]",
                        "name": "DuckDuckGo Search",
                        "parameters": {
                            "properties": {
                                "message": {
                                    "description": "The search query to send to DuckDuckGo",
                                    "type": "string"
                                }
                            },
                            "required": ["message"],
                            "type": "object"
                        },
                        "tool_router_key": "local:::duckduckgo_search:::duckduckgo_search"
                    },
                    "type": "function"
                }),
                98,
            ),
            SubPrompt::ToolAvailable(
                SubPromptType::AvailableTool,
                serde_json::json!({
                    "function": {
                        "description": "This function takes a question as input and returns a comprehensive answer, along with the sources and statements used to generate the answer.",
                        "name": "Smart Search Engine",
                        "parameters": {
                            "properties": {
                                "question": {
                                    "description": "The question to answer",
                                    "type": "string"
                                }
                            },
                            "required": ["question"],
                            "type": "object"
                        },
                        "tool_router_key": "local:::smart_search_shinkai:::smart_search_engine"
                    },
                    "type": "function"
                }),
                97,
            ),
            SubPrompt::Omni(
                SubPromptType::UserLastMessage,
                "search in duckduckgo for movies".to_string(),
                vec![],
                100,
            ),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        let model = SerializedLLMProvider::mock_provider().model;

        let (result, system_messages) = claude_prepare_messages(&model, prompt).unwrap();
        let messages = match result.messages {
            PromptResultEnum::Value(v) => v,
            _ => panic!("Expected Value variant"),
        };

        // Verify system message
        assert_eq!(system_messages.len(), 1);
        assert_eq!(system_messages[0].role, Some("system".to_string()));

        // Verify user message
        let user_message = messages
            .as_array()
            .unwrap()
            .iter()
            .find(|msg| msg.get("role") == Some(&json!("user")) && msg.get("content").is_some())
            .expect("Should have found a user message");

        assert_eq!(
            user_message.get("content").unwrap().as_str().unwrap(),
            "search in duckduckgo for movies"
        );

        // Verify functions are present in the result
        assert!(result.functions.is_some());
        let functions = result.functions.unwrap();
        assert_eq!(functions.len(), 3); // Should have all three tools available

        // Verify each function name follows the pattern and matches expected sanitized name
        let name_pattern = Regex::new(r"^[a-zA-Z0-9_-]{1,64}$").unwrap();

        let function_names: Vec<String> = functions
            .iter()
            .map(|f| {
                f.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n.to_string())
                    .expect("Each function should have a name")
            })
            .collect();

        // Print all function names for debugging
        eprintln!("Found function names: {:?}", function_names);

        // Verify each name matches the pattern
        for name in &function_names {
            assert!(
                name_pattern.is_match(name),
                "Tool name '{}' should match pattern ^[a-zA-Z0-9_-]{{1,64}}$",
                name
            );
        }

        // Verify the expected sanitized names are present
        assert!(
            function_names.contains(&"google_search".to_string()),
            "Should contain sanitized Google Search tool"
        );
        assert!(
            function_names.contains(&"duckduckgo_search".to_string()),
            "Should contain sanitized DuckDuckGo Search tool"
        );
        assert!(
            function_names.contains(&"smart_search_engine".to_string()),
            "Should contain sanitized Smart Search Engine tool"
        );
    }
}
