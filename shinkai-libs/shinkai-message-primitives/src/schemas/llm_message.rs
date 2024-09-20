use serde::{Deserialize, Serialize};
use serde_json::{Error as SerdeError, Value};
use thiserror::Error;

/// The parameters of the function.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionParameters {
    /// The type of the parameters.
    #[serde(rename = "type")]
    pub type_: String,
    /// The properties of the parameters.
    pub properties: Value,
    /// The required parameters.
    pub required: Vec<String>,
}

/// The details of the function to be called.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionDetails {
    /// The name of the function.
    pub name: String,
    /// The description of the function.
    pub description: String,
    /// The parameters of the function.
    pub parameters: FunctionParameters,
}

/// The structure for a function call with detailed parameters.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailedFunctionCall {
    /// The name of the function.
    pub name: String,
    /// The arguments of the function call.
    pub arguments: String,
}

/// The message structure for LLM communication.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmMessage {
    /// The role of the message's author. One of `system`, `user`, `assistant`, or `function`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The contents of the message.
    /// `content` is required for all messages except assistant messages with function calls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// The name of the author of this message. `name` is required if role is function,
    /// and it should be the name of the function whose response is in the `content`.
    /// May contain a-z, A-Z, 0-9, and underscores, with a maximum length of 64 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The detailed function call structure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<DetailedFunctionCall>,
    /// The available functions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<FunctionDetails>>,
    /// The images associated with the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

#[derive(Debug, Error)]
pub enum LlmMessageError {
    #[error("Failed to parse JSON: {0}")]
    JsonParseError(#[from] SerdeError),
}

impl LlmMessage {
    /// Imports an LlmMessage from a JSON value.
    pub fn import_functions_from_value(value: Value) -> Result<Self, LlmMessageError> {
        let role = None;
        let content = None;
        let name = None;

        let images = value.get("images").and_then(|v| v.as_array().map(|arr| {
            arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
        }));

        // Extract the functions from the "function" key
        let functions_value = value.get("function").ok_or_else(|| {
            let err_msg = "Missing 'function' key in JSON";
            eprintln!("{}", err_msg);
            LlmMessageError::JsonParseError(serde::de::Error::custom(err_msg))
        })?;

        let functions: Vec<FunctionDetails> = match functions_value {
            Value::Array(arr) => serde_json::from_value(Value::Array(arr.to_vec()))?,
            Value::Object(_) => vec![serde_json::from_value(functions_value.clone())?],
            _ => {
                return Err(LlmMessageError::JsonParseError(serde::de::Error::custom(
                    "Invalid JSON format for functions",
                )))
            }
        };

        Ok(LlmMessage {
            role,
            content,
            name,
            function_call: None,
            functions: Some(functions),
            images,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_import_functions_from_value_object() {
        let json_value = json!({
            "function": {
                "name": "concat_strings",
                "description": "Concatenates 2 to 4 strings.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "first_string": {
                            "type": "string",
                            "description": "The first string to concatenate"
                        },
                        "second_string": {
                            "type": "string",
                            "description": "The second string to concatenate"
                        },
                        "third_string": {
                            "type": "string",
                            "description": "The third string to concatenate (optional)"
                        },
                        "fourth_string": {
                            "type": "string",
                            "description": "The fourth string to concatenate (optional)"
                        }
                    },
                    "required": ["first_string", "second_string"]
                }
            },
            "type": "function"
        });

        let message =
            LlmMessage::import_functions_from_value(json_value).expect("Failed to import functions from value");

        assert!(message.role.is_none());
        assert!(message.content.is_none());
        assert!(message.name.is_none());
        assert!(message.function_call.is_none());
        assert!(message.functions.is_some());

        let functions = message.functions.unwrap();
        assert_eq!(functions.len(), 1);

        let function = &functions[0];
        assert_eq!(function.name, "concat_strings");
        assert_eq!(function.description, "Concatenates 2 to 4 strings.");
        assert_eq!(function.parameters.type_, "object");

        let properties = function.parameters.properties.as_object().unwrap();
        assert_eq!(properties.get("first_string").unwrap().get("type").unwrap(), "string");
        assert_eq!(
            properties.get("first_string").unwrap().get("description").unwrap(),
            "The first string to concatenate"
        );
        assert_eq!(properties.get("second_string").unwrap().get("type").unwrap(), "string");
        assert_eq!(
            properties.get("second_string").unwrap().get("description").unwrap(),
            "The second string to concatenate"
        );
        assert_eq!(properties.get("third_string").unwrap().get("type").unwrap(), "string");
        assert_eq!(
            properties.get("third_string").unwrap().get("description").unwrap(),
            "The third string to concatenate (optional)"
        );
        assert_eq!(properties.get("fourth_string").unwrap().get("type").unwrap(), "string");
        assert_eq!(
            properties.get("fourth_string").unwrap().get("description").unwrap(),
            "The fourth string to concatenate (optional)"
        );

        let required = &function.parameters.required;
        assert_eq!(required.len(), 2);
        assert!(required.contains(&"first_string".to_string()));
        assert!(required.contains(&"second_string".to_string()));
    }

    #[test]
    fn test_llm_message_from_json_value() {
        let json_value = json!({
            "role": "assistant",
            "content": null,
            "function_call": {
                "name": "concat_strings",
                "arguments": "{\"first_string\":\"hola\",\"second_string\":\"chao\"}"
            }
        });

        let message: LlmMessage =
            serde_json::from_value(json_value).expect("Failed to convert JSON value to LlmMessage");

        assert_eq!(message.role, Some("assistant".to_string()));
        assert!(message.content.is_none());
        assert!(message.name.is_none());
        assert!(message.functions.is_none());

        let function_call = message.function_call.unwrap();
        assert_eq!(function_call.name, "concat_strings");
        assert_eq!(
            function_call.arguments,
            "{\"first_string\":\"hola\",\"second_string\":\"chao\"}"
        );
    }

    #[test]
    fn test_llm_message_from_json_value_with_images() {
        let json_value = json!({
            "role": "user",
            "content": "This is a test message",
            "images": ["image1", "image2"],
            "function_call": {
                "name": "concat_strings",
                "arguments": "{\"first_string\":\"hola\",\"second_string\":\"chao\"}"
            }
        });

        let message: LlmMessage =
            serde_json::from_value(json_value).expect("Failed to convert JSON value to LlmMessage");

        assert_eq!(message.role, Some("user".to_string()));
        assert_eq!(message.content, Some("This is a test message".to_string()));
        assert!(message.name.is_none());
        assert!(message.functions.is_none());

        let function_call = message.function_call.unwrap();
        assert_eq!(function_call.name, "concat_strings");
        assert_eq!(
            function_call.arguments,
            "{\"first_string\":\"hola\",\"second_string\":\"chao\"}"
        );

        let images = message.images.unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0], "image1");
        assert_eq!(images[1], "image2");
    }
}
