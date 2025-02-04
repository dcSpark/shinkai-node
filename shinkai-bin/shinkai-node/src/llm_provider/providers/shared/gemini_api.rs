use super::shared_model_logic::get_image_type;
use crate::llm_provider::error::LLMProviderError;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::model_capabilities_manager::PromptResult;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use serde_json::{self};
use shinkai_message_primitives::schemas::llm_message::FunctionParameters;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use shinkai_message_primitives::schemas::prompts::Prompt;

pub fn gemini_prepare_messages(model: &LLMProviderInterface, prompt: Prompt) -> Result<PromptResult, LLMProviderError> {
    eprintln!("Preparing messages for Gemini... {:?}", prompt);

    let max_input_tokens = ModelCapabilitiesManager::get_max_input_tokens(model);

    // Generate the messages and filter out images
    let chat_completion_messages = prompt.generate_llm_messages(
        Some(max_input_tokens),
        None,
        &ModelCapabilitiesManager::num_tokens_from_llama3,
    )?;

    // Get a more accurate estimate of the number of used tokens
    let used_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&chat_completion_messages);
    // Calculate the remaining output tokens available
    let remaining_output_tokens = ModelCapabilitiesManager::get_remaining_output_tokens(model, used_tokens);

    // Separate messages into those with a user / assistant / system role and those without
    let (mut messages_with_role, tools): (Vec<_>, Vec<_>) = chat_completion_messages
        .into_iter()
        .partition(|message| message.role.is_some());

    // Update the role from "assistant" to "model"
    for message in &mut messages_with_role {
        if let Some(role) = &message.role {
            if role == "assistant" {
                message.role = Some("model".to_string());
            }
        }
    }

    // Convert both sets of messages to serde Value
    let messages_json = serde_json::to_value(messages_with_role)?;

    // Convert messages_json to Vec<serde_json::Value>
    let messages_vec = match messages_json {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .map(|mut message| {
                let images = message.get("images").cloned();
                let content = message.get("content").cloned();

                // If this had images, turn them into inline_data
                if let Some(serde_json::Value::Array(images_array)) = images {
                    let mut parts = vec![];
                    if let Some(text) = content {
                        if let Some(text_str) = text.as_str() {
                            if !text_str.is_empty() {
                                parts.push(serde_json::json!({"type": "text", "text": text}));
                            }
                        }
                    }
                    for image in images_array {
                        if let serde_json::Value::String(image_str) = image {
                            if let Some(image_type) = get_image_type(&image_str) {
                                parts.push(serde_json::json!({
                                    "inline_data": {
                                        "mime_type": format!("image/{}", image_type),
                                        "data": image_str
                                    }
                                }));
                            }
                        }
                    }
                    message["content"] = serde_json::json!(parts);
                    message.as_object_mut().unwrap().remove("images");
                }
                message
            })
            .collect(),
        _ => vec![],
    };

    // Now build the final structure from messages_vec
    let messages_vec = messages_vec
        .into_iter()
        .map(|msg| {
            let role = if msg.get("role") == Some(&serde_json::Value::String("function".to_string())) {
                serde_json::Value::String("user".to_string())
            } else {
                msg.get("role")
                    .cloned()
                    .unwrap_or(serde_json::Value::String("".to_string()))
            };
            let content = msg
                .get("content")
                .cloned()
                .unwrap_or(serde_json::Value::String("".to_string()));

            // Check if this is a "function_response" field (the older logic)
            if let Some(function_response) = msg.get("function_response") {
                let response = function_response
                    .get("response")
                    .and_then(|v| {
                        if let serde_json::Value::String(s) = v {
                            serde_json::from_str(s).ok()
                        } else {
                            Some(v.clone())
                        }
                    })
                    .unwrap_or(serde_json::json!({}));

                serde_json::json!({
                    "role": "user",
                    "parts": [{
                        "text": response.to_string()
                    }]
                })
            }
            // Check if this is a "function_call" field with a response
            else if let Some(function_call_val) = msg.get("function_call") {
                let name = function_call_val
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();

                // Parse the arguments
                let args = function_call_val
                    .get("arguments")
                    .and_then(|v| {
                        if let serde_json::Value::String(s) = v {
                            serde_json::from_str(s).ok()
                        } else {
                            Some(v.clone())
                        }
                    })
                    .unwrap_or(serde_json::json!({}));

                // If there is a "response" field inside function_call,
                // treat it as a function response, not a function call
                if let Some(response_val) = function_call_val.get("response") {
                    let parsed_response = if let serde_json::Value::String(s) = response_val {
                        serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!({}))
                    } else {
                        response_val.clone()
                    };

                    serde_json::json!({
                        "role": "user",
                        "parts": [{
                            "text": parsed_response.to_string()
                        }]
                    })
                } else {
                    // Normal function call
                    serde_json::json!({
                        "role": role,
                        "parts": [{
                            "functionCall": {
                                "name": name,
                                "args": args
                            }
                        }]
                    })
                }
            }
            // Regular message
            else {
                let content = match content {
                    serde_json::Value::String(text) => {
                        if text.is_empty() {
                            vec![]
                        } else {
                            vec![serde_json::json!({"text": text})]
                        }
                    }
                    serde_json::Value::Array(arr) => {
                        let mut parts = vec![];
                        for item in arr {
                            if let serde_json::Value::Object(mut obj) = item {
                                if let Some(serde_json::Value::String(text)) = obj.remove("text") {
                                    if !text.is_empty() {
                                        parts.push(serde_json::json!({"text": text}));
                                    }
                                }
                                if let Some(serde_json::Value::Object(inline_data)) = obj.remove("inline_data") {
                                    parts.push(serde_json::json!({"inline_data": inline_data}));
                                }
                            }
                        }
                        parts
                    }
                    _ => vec![], // If content is neither string nor array
                };

                if content.is_empty() {
                    serde_json::json!({})
                } else {
                    serde_json::json!({
                        "role": role,
                        "parts": content
                    })
                }
            }
        })
        .filter(|msg| !msg.as_object().map_or(true, |obj| obj.is_empty()))
        .collect::<Vec<_>>();

    // Convert tools to function declarations
    let function_declarations: Vec<serde_json::Value> = tools
        .into_iter()
        .filter_map(|tool| tool.functions)
        .flatten()
        .map(|mut function| {
            // Fix up the parameters
            fix_function_parameters(&mut function.parameters);

            // Special handling for typescript processor parameters
            if function.name == "shinkai_typescript_unsafe_processor" {
                if let Some(props) = function.parameters.properties.as_object_mut() {
                    // Fix the config parameter
                    if let Some(config) = props.get_mut("config") {
                        if let Some(config_obj) = config.as_object_mut() {
                            config_obj.insert(
                                "properties".to_string(),
                                serde_json::json!({
                                    "options": {
                                        "type": "object",
                                        "description": "Configuration options",
                                        "properties": {
                                            "timeout": {
                                                "type": "number",
                                                "description": "Execution timeout in milliseconds"
                                            }
                                        }
                                    }
                                }),
                            );
                        }
                    }

                    // Fix the parameters parameter
                    if let Some(params) = props.get_mut("parameters") {
                        if let Some(params_obj) = params.as_object_mut() {
                            params_obj.insert(
                                "properties".to_string(),
                                serde_json::json!({
                                    "args": {
                                        "type": "object",
                                        "description": "Function arguments",
                                        "properties": {
                                            "input": {
                                                "type": "string",
                                                "description": "Input data"
                                            }
                                        }
                                    }
                                }),
                            );
                        }
                    }
                }
            }

            serde_json::json!({
                "name": function.name.chars()
                    .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c.to_ascii_lowercase() } else { '_' })
                    .collect::<String>(),
                "description": function.description,
                "parameters": function.parameters
            })
        })
        .collect();

    // Separate system instruction from other messages
    let system_instruction = messages_vec
        .iter()
        .find(|msg| msg.get("role") == Some(&serde_json::Value::String("system".to_string())))
        .and_then(|msg| {
            msg.get("parts")
                .and_then(|parts| parts.as_array())
                .and_then(|parts| parts.first())
                .and_then(|part| part.get("text"))
                .and_then(|text| text.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    let contents: Vec<_> = messages_vec
        .iter()
        .filter(|msg| msg.get("role") != Some(&serde_json::Value::String("system".to_string())))
        .cloned()
        .collect();

    let mut result_json = serde_json::json!({
        "system_instruction": {
            "parts": { "text": system_instruction }
        },
        "contents": contents
    });

    // Add tools and tool_config if there are function declarations
    if !function_declarations.is_empty() {
        if let Some(obj) = result_json.as_object_mut() {
            obj.insert(
                "tools".to_string(),
                serde_json::json!([{
                    "function_declarations": function_declarations
                }]),
            );
            obj.insert(
                "tool_config".to_string(),
                serde_json::json!({
                    "function_calling_config": {
                        "mode": "AUTO"
                    }
                }),
            );
        }
    }

    Ok(PromptResult {
        messages: PromptResultEnum::Value(result_json),
        functions: Some(vec![]),
        remaining_output_tokens,
        tokens_used: used_tokens,
    })
}

fn fix_function_parameters(params: &mut FunctionParameters) {
    // If this parameter is declared as an object...
    if params.type_ == "object" {
        // Safely get a mutable reference to its properties object.
        if let Some(props_obj) = params.properties.as_object_mut() {
            // If there are zero declared properties...
            if props_obj.is_empty() {
                // Insert a dummy property so Gemini doesn't reject the schema.
                props_obj.insert(
                    "dummy".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Dummy property to satisfy Gemini's requirement for non-empty object schemas"
                    }),
                );
            } else {
                // If there *are* properties, fix them recursively in case any sub-property is also an empty object
                for (_prop_name, prop_value) in props_obj.iter_mut() {
                    if let Some(sub_obj) = prop_value.as_object_mut() {
                        // If sub_obj has "type":"object", do the same check
                        let is_object = sub_obj
                            .get("type")
                            .and_then(|v| v.as_str())
                            .map_or(false, |s| s == "object");

                        if is_object {
                            // Ensure it has a properties field
                            if !sub_obj.contains_key("properties") {
                                sub_obj.insert(
                                    "properties".to_string(),
                                    serde_json::json!({
                                        "dummy": {
                                            "type": "string",
                                            "description": "Dummy property to satisfy Gemini's requirement for non-empty object schemas"
                                        }
                                    }),
                                );
                            } else if let Some(props) = sub_obj.get_mut("properties") {
                                if let Some(nested_props) = props.as_object_mut() {
                                    if nested_props.is_empty() {
                                        nested_props.insert(
                                            "dummy".to_string(),
                                            serde_json::json!({
                                                "type": "string",
                                                "description": "Dummy property to satisfy Gemini's requirement for non-empty object schemas"
                                            }),
                                        );
                                    } else {
                                        // Recursively fix any nested object properties
                                        for nested in nested_props.values_mut() {
                                            if let Some(nested_obj) = nested.as_object_mut() {
                                                let nested_is_object = nested_obj
                                                    .get("type")
                                                    .and_then(|v| v.as_str())
                                                    .map_or(false, |s| s == "object");
                                                if nested_is_object {
                                                    fix_object_in_place(nested_obj);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Helper for fixing deeply nested objects
fn fix_object_in_place(obj: &mut serde_json::Map<String, serde_json::Value>) {
    let props_opt = obj.get_mut("properties");
    if let Some(props_val) = props_opt {
        if let Some(nested_props) = props_val.as_object_mut() {
            if nested_props.is_empty() {
                nested_props.insert(
                    "dummy".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Deep nested dummy property"
                    }),
                );
            } else {
                for nested in nested_props.values_mut() {
                    if let Some(nested_obj) = nested.as_object_mut() {
                        let nested_is_object = nested_obj
                            .get("type")
                            .and_then(|v| v.as_str())
                            .map_or(false, |s| s == "object");
                        if nested_is_object {
                            fix_object_in_place(nested_obj);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
    use shinkai_message_primitives::schemas::subprompts::{SubPrompt, SubPromptAssetType, SubPromptType};

    fn generate_test_payload(
        prompt: Prompt,
        model: &LLMProviderInterface,
    ) -> Result<serde_json::Value, LLMProviderError> {
        let result = gemini_prepare_messages(model, prompt)?;
        let contents = match result.messages {
            PromptResultEnum::Value(v) => v,
            _ => {
                return Err(LLMProviderError::UnexpectedPromptResultVariant(
                    "Expected Value variant in PromptResultEnum".to_string(),
                ))
            }
        };

        let mut payload = json!({
            "generationConfig": {
                "temperature": 0.9,
                "topK": 1,
                "topP": 1,
                "maxOutputTokens": 8192
            },
            "safety_settings": [
                {
                    "category": "HARM_CATEGORY_DANGEROUS_CONTENT",
                    "threshold": "BLOCK_NONE"
                },
                {
                    "category": "HARM_CATEGORY_HARASSMENT",
                    "threshold": "BLOCK_NONE"
                },
                {
                    "category": "HARM_CATEGORY_HATE_SPEECH",
                    "threshold": "BLOCK_NONE"
                },
                {
                    "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT",
                    "threshold": "BLOCK_NONE"
                }
            ],
            "tool_config": {
                "function_calling_config": {
                    "mode": "AUTO"
                }
            }
        });

        if let Some(payload_obj) = payload.as_object_mut() {
            if let Some(contents_obj) = contents.as_object() {
                for (key, value) in contents_obj {
                    payload_obj.insert(key.clone(), value.clone());
                }
            }
        }

        Ok(payload)
    }

    #[test]
    fn test_gemini_from_llm_messages() {
        let sub_prompts = vec![
            SubPrompt::Omni(
                SubPromptType::System,
                "You are Neko the cat respond like one".to_string(),
                vec![],
                98,
            ),
            SubPrompt::Omni(SubPromptType::User, "Hello".to_string(), vec![], 97),
            SubPrompt::Omni(
                SubPromptType::Assistant,
                "Great to meet you. What would you like to know?".to_string(),
                vec![],
                97,
            ),
            SubPrompt::Omni(
                SubPromptType::User,
                "I have two dogs in my house. How many paws are in my house?".to_string(),
                vec![(
                    SubPromptAssetType::Image,
                    "iVBORw0KGgoAAAANSUhEUgAAAQAAAAEACAMAAABrrFhUAAAABGdBTUEAALGPC/xhBQAAAAFzUkdCAK7OHOkAAAD5UExURQAAAACl7QCl7ACm7ACl7ACl7ACm7QCm7QCm7QCm7ACm7ACm7QCl7QCl6wCl7ACl7ACl7ACl7QCl6wCl7QCl7ACm7ACm7ACl7QCm7ACl7QCl6wCm7QCm7QCl7ACl7QCm7QCl7QCl7QCm7ACm7QCl6wCl7ACl7QCl7ACm7ACm7ACl7QCl7ACl7QCm7QCm7ACm7ACl7ACl7QCl6wCm7QCm6wCm7QCm7QCm7QCl7QCl7ACm7QCl7ACm7QCl7QCl7ACk6wCl7QCl7ACm7ACl7QCm7ACl7QCl7ACm7QCl7ACm7ACm7ACm7QCl7ACl7ACm7QCl7QCk7ACm7ACm7ahktTwAAABSdFJOUwDoJJubI+v7+vaN9fswD50JzFrLCCbo+esOCcvUWZvM6S9Z+YzP0cyc0VrriQ6MCCX0JIoK7J5Z9p6ZDi8PiCPr6NMl1CTRJSbn+p2cWiSgD4gNsVXUAAACIElEQVR42u3X11KVMRiG0Wx2+femN+kgiL1Ls4MKKhZQc/8X4xmnye+BM3yznjt4VyaTSUqSJEmSJElX/Z7/ObfS5Gtes3Kz+/D9P8y/MTPKYXr167zl/LXxYQ7V8OlBm/1jGzlcZ3v1+98t5YB1pqrPv5NDtj1Wef83ctC+LFYBjOewPa56/4ZxAXYvKwBe58BtlfdPjyIDTKwWAeZz6DaLAPdjA/SKAPdiAywUAQ5jAwyKAE1sgMkiQA4eAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA8D8Amtj7J4sAg9gAj4oAc7EB1osAvdgA3SLARWyAW0WA6VHk/ROrRYA0ExngR3l/ej6Mu3/5tAIg3Y4L8Kdmf5r9FHX/g2dVAKn/Meb+pX6qbOdbxP2dqVRd/068/XdPUosWj5djzf++P5va9fnNizjzR0dfU/uevOyuf7j2v+NmsNDbfJskSZIkSZKu+gtLvn0aIyUzCwAAAABJRU5ErkJggg==".to_string(),
                    "image.png".to_string(),
                )],
                100,
            ),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        // Use the mock provider
        let model = SerializedLLMProvider::mock_provider().model;

        // Call the gemini_prepare_messages function
        let result = gemini_prepare_messages(&model, prompt.clone()).expect("Failed to prepare messages");

        // Define the expected messages and functions
        let expected_messages = json!({
            "system_instruction": {
                "parts": { "text": "You are Neko the cat respond like one" }
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": "Hello" }]
                },
                {
                    "role": "model",
                    "parts": [{ "text": "Great to meet you. What would you like to know?" }]
                },
                {
                    "role": "user",
                    "parts": [{ "text": "I have two dogs in my house. How many paws are in my house?" },
                    {
                        "inline_data": {
                            "mime_type": "image/png",
                            "data": "iVBORw0KGgoAAAANSUhEUgAAAQAAAAEACAMAAABrrFhUAAAABGdBTUEAALGPC/xhBQAAAAFzUkdCAK7OHOkAAAD5UExURQAAAACl7QCl7ACm7ACl7ACl7ACm7QCm7QCm7QCm7ACm7ACm7QCl7QCl6wCl7ACl7ACl7ACl7QCl6wCl7QCl7ACm7ACm7ACl7QCm7ACl7QCl6wCm7QCm7QCl7ACl7QCm7QCl7QCl7QCm7ACm7QCl6wCl7ACl7QCl7ACm7ACm7ACl7QCl7ACl7QCm7QCm7ACm7ACl7ACl7QCl6wCm7QCm6wCm7QCm7QCm7QCl7QCl7ACm7QCl7ACm7QCl7QCl7ACk6wCl7QCl7ACm7ACl7QCm7ACl7QCl7ACm7QCl7ACm7ACm7ACm7QCl7ACl7ACm7QCl7QCk7ACm7ACm7ahktTwAAABSdFJOUwDoJJubI+v7+vaN9fswD50JzFrLCCbo+esOCcvUWZvM6S9Z+YzP0cyc0VrriQ6MCCX0JIoK7J5Z9p6ZDi8PiCPr6NMl1CTRJSbn+p2cWiSgD4gNsVXUAAACIElEQVR42u3X11KVMRiG0Wx2+femN+kgiL1Ls4MKKhZQc/8X4xmnye+BM3yznjt4VyaTSUqSJEmSJElX/Z7/ObfS5Gtes3Kz+/D9P8y/MTPKYXr167zl/LXxYQ7V8OlBm/1jGzlcZ3v1+98t5YB1pqrPv5NDtj1Wef83ctC+LFYBjOewPa56/4ZxAXYvKwBe58BtlfdPjyIDTKwWAeZz6DaLAPdjA/SKAPdiAywUAQ5jAwyKAE1sgMkiQA4eAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA8D8Amtj7J4sAg9gAj4oAc7EB1osAvdgA3SLARWyAW0WA6VHk/ROrRYA0ExngR3l/ej6Mu3/5tAIg3Y4L8Kdmf5r9FHX/g2dVAKn/Meb+pX6qbOdbxP2dqVRd/068/XdPUosWj5djzf++P5va9fnNizjzR0dfU/uevOyuf7j2v+NmsNDbfJskSZIkSZKu+gtLvn0aIyUzCwAAAABJRU5ErkJggg=="
                        }
                    }]
                }
            ],
            // "functions": []
        });

        // Assert the results
        assert_eq!(result.messages, PromptResultEnum::Value(expected_messages));
        assert!(result.remaining_output_tokens > 0);

        // Generate and print the final payload
        let payload = generate_test_payload(prompt, &model).expect("Failed to generate payload");
        println!("Final Payload: {}", serde_json::to_string_pretty(&payload).unwrap());
    }

    #[test]
    fn test_gemini_with_duckduckgo_search() {
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
                    },
                    "type": "function"
                }),
                98,
            ),
            SubPrompt::Omni(
                SubPromptType::UserLastMessage,
                "\nduckduckgo search for movies".to_string(),
                vec![],
                100,
            ),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        let model = SerializedLLMProvider::mock_provider().model;
        let result = gemini_prepare_messages(&model, prompt.clone()).expect("Failed to prepare messages");

        let expected_messages = json!({
            "system_instruction": {
                "parts": { "text": "You are a very helpful assistant. You may be provided with documents or content to analyze and answer questions about them, in that case refer to the content provided in the user message for your responses." }
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": "duckduckgo search for movies" }]
                }
            ],
            "tools": [{
                "function_declarations": [{
                    "name": "duckduckgo_search",
                    "description": "Searches the DuckDuckGo search engine. Example result: [{\"title\": \"IMDb Top 250 Movies\", \"description\": \"Find out which <b>movies</b> are rated as the <b>best</b> <b>of</b> <b>all</b> <b>time</b> by IMDb users. See the list of 250 titles sorted by ranking, genre, year, and rating, and learn how the list is determined.\", \"url\": \"https://www.imdb.com/chart/top/\"}]",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "message": {
                                "type": "string",
                                "description": "The search query to send to DuckDuckGo"
                            }
                        },
                        "required": ["message"]
                    }
                }]
            }],
            "tool_config": {
                "function_calling_config": {
                    "mode": "AUTO"
                }
            }
        });

        assert_eq!(result.messages, PromptResultEnum::Value(expected_messages));
        assert!(result.remaining_output_tokens > 0);

        // Generate and print the final payload
        let payload = generate_test_payload(prompt, &model).expect("Failed to generate payload");
        println!("Final Payload: {}", serde_json::to_string_pretty(&payload).unwrap());
    }

    #[test]
    fn test_gemini_with_duckduckgo_search_and_response() {
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
        let result = gemini_prepare_messages(&model, prompt.clone()).expect("Failed to prepare messages");

        let expected_messages = json!({
            "system_instruction": {
                "parts": { "text": "You are a very helpful assistant. You may be provided with documents or content to analyze and answer questions about them, in that case refer to the content provided in the user message for your responses." }
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": "duckduckgo search for movies" }]
                },
                {
                    "role": "model",
                    "parts": [{
                        "functionCall": {
                            "name": "duckduckgo_search",
                            "args": {
                                "message": "movies"
                            }
                        }
                    }]
                },
                {
                    "role": "user",
                    "parts": [{
                        "text": "{\"data\":{\"__created_files__\":[\"shinkai://file/@@my_local_ai.sep-shinkai/main/jobid_c93837a6-358b-4648-9617-1d6e93d0bb59/logs/log_jobid_c93837a6-358b-4648-9617-1d6e93d0bb59_localduckduckgo_searchduckduckgo_search.log\"],\"message\":\"[{\\\"title\\\":\\\"Movie Tickets &amp; Movie Times | Fandango\\\",\\\"description\\\":\\\"Honoring the Best <b>movies</b> &amp; TV. Check out the winners from this year&#x27;s 26th annual Rotten Tomatoes Awards. LEARN MORE. Collectors, assemble! image link. Collectors, assemble! Suit up and get the Captain America Collector Pack, featuring an exclusive Collector&#x27;s Coin, Limited-Edition Poster, and one <b>movie</b> ticket!\\\",\\\"url\\\":\\\"https://www.fandango.com/\\\"}]\"}}",
                    }]
                }
            ],
            "tools": [{
                "function_declarations": [{
                    "name": "duckduckgo_search",
                    "description": "Searches the DuckDuckGo search engine. Example result: [{\"title\": \"IMDb Top 250 Movies\", \"description\": \"Find out which <b>movies</b> are rated as the <b>best</b> <b>of</b> <b>all</b> <b>time</b> by IMDb users. See the list of 250 titles sorted by ranking, genre, year, and rating, and learn how the list is determined.\", \"url\": \"https://www.imdb.com/chart/top/\"}]",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "message": {
                                "type": "string",
                                "description": "The search query to send to DuckDuckGo"
                            }
                        },
                        "required": ["message"]
                    }
                }]
            }],
            "tool_config": {
                "function_calling_config": {
                    "mode": "AUTO"
                }
            }
        });

        // Generate and print the final payload
        let payload = generate_test_payload(prompt, &model).expect("Failed to generate payload");
        println!("Final Payload: {}", serde_json::to_string_pretty(&payload).unwrap());

        assert_eq!(result.messages, PromptResultEnum::Value(expected_messages));
        assert!(result.remaining_output_tokens > 0);
    }

    #[test]
    fn test_gemini_with_typescript_processor() {
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
                        "description": "Tool for executing Node.js code. This is unsafe and should be used with extreme caution.",
                        "name": "shinkai_typescript_unsafe_processor",
                        "parameters": {
                            "properties": {
                                "code": {
                                    "description": "The TypeScript code to execute",
                                    "type": "string"
                                },
                                "config": {
                                    "description": "Configuration for the code execution",
                                    "type": "object",
                                    "properties": {
                                        "options": {
                                            "type": "object",
                                            "description": "Configuration options",
                                            "properties": {
                                                "timeout": {
                                                    "type": "number",
                                                    "description": "Execution timeout in milliseconds"
                                                }
                                            }
                                        }
                                    }
                                },
                                "package": {
                                    "description": "The package.json contents",
                                    "type": "string"
                                },
                                "parameters": {
                                    "description": "Parameters to pass to the code",
                                    "type": "object",
                                    "properties": {
                                        "args": {
                                            "type": "object",
                                            "description": "Function arguments",
                                            "properties": {
                                                "input": {
                                                    "type": "string",
                                                    "description": "Input data"
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            "required": ["code", "package", "parameters", "config"],
                            "type": "object"
                        }
                    },
                    "type": "function"
                }),
                98,
            ),
            SubPrompt::Omni(
                SubPromptType::UserLastMessage,
                "Run some TypeScript code".to_string(),
                vec![],
                100,
            ),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        let model = SerializedLLMProvider::mock_provider().model;
        let result = gemini_prepare_messages(&model, prompt.clone()).expect("Failed to prepare messages");

        let expected_messages = json!({
            "system_instruction": {
                "parts": { "text": "You are a very helpful assistant. You may be provided with documents or content to analyze and answer questions about them, in that case refer to the content provided in the user message for your responses." }
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": "Run some TypeScript code" }]
                }
            ],
            "tools": [{
                "function_declarations": [{
                    "name": "shinkai_typescript_unsafe_processor",
                    "description": "Tool for executing Node.js code. This is unsafe and should be used with extreme caution.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "code": {
                                "description": "The TypeScript code to execute",
                                "type": "string"
                            },
                            "config": {
                                "description": "Configuration for the code execution",
                                "type": "object",
                                "properties": {
                                    "options": {
                                        "type": "object",
                                        "description": "Configuration options",
                                        "properties": {
                                            "timeout": {
                                                "type": "number",
                                                "description": "Execution timeout in milliseconds"
                                            }
                                        }
                                    }
                                }
                            },
                            "package": {
                                "description": "The package.json contents",
                                "type": "string"
                            },
                            "parameters": {
                                "description": "Parameters to pass to the code",
                                "type": "object",
                                "properties": {
                                    "args": {
                                        "type": "object",
                                        "description": "Function arguments",
                                        "properties": {
                                            "input": {
                                                "type": "string",
                                                "description": "Input data"
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        "required": ["code", "package", "parameters", "config"]
                    }
                }]
            }],
            "tool_config": {
                "function_calling_config": {
                    "mode": "AUTO"
                }
            }
        });

        assert_eq!(result.messages, PromptResultEnum::Value(expected_messages));
        assert!(result.remaining_output_tokens > 0);

        // Generate and print the final payload
        let payload = generate_test_payload(prompt, &model).expect("Failed to generate payload");
        println!("Final Payload: {}", serde_json::to_string_pretty(&payload).unwrap());
    }
}
