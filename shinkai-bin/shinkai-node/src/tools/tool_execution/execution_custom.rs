use serde_json::{Map, Value, json};
use shinkai_tools_primitives::tools::error::ToolError;

pub fn execute_custom_tool(    
    tool_router_key: &String,
    parameters: Map<String, Value>,
    extra_config: Option<String>,) -> Option<Result<Value, ToolError>> {
    // Get the tool name from the parameters or router key
    
    match tool_router_key {
        s if s == &String::from("internal:::calculator") => Some(execute_calculator(&parameters)),
        s if s == &String::from("internal:::text_analyzer") => Some(execute_text_analyzer(&parameters)),
        _ => None, // Not a custom tool
    }
}

fn execute_calculator(parameters: &Map<String, Value>) -> Result<Value, ToolError> {
    // Extract parameters
    let operation = parameters.get("operation")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::SerializationError("Missing operation parameter".to_string()))?;
    
    let x = parameters.get("x")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| ToolError::SerializationError("Missing or invalid x parameter".to_string()))?;
    
    let y = parameters.get("y")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| ToolError::SerializationError("Missing or invalid y parameter".to_string()))?;

    // Perform calculation
    let result = match operation {
        "add" => x + y,
        "subtract" => x - y,
        "multiply" => x * y,
        "divide" => {
            if y == 0.0 {
                return Err(ToolError::ExecutionError("Division by zero".to_string()));
            }
            x / y
        },
        _ => return Err(ToolError::ExecutionError("Invalid operation".to_string())),
    };

    Ok(json!({
        "result": result
    }))
}

fn execute_text_analyzer(parameters: &Map<String, Value>) -> Result<Value, ToolError> {
    // Extract parameters
    let text = parameters.get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::SerializationError("Missing text parameter".to_string()))?;
    
    let include_sentiment = parameters.get("include_sentiment")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Calculate basic statistics
    let word_count = text.split_whitespace().count();
    let character_count = text.chars().count();

    // Create response
    let mut response = json!({
        "word_count": word_count,
        "character_count": character_count,
    });

    // Add sentiment analysis if requested
    if include_sentiment {
        let sentiment_score = calculate_mock_sentiment(text);
        response.as_object_mut().unwrap().insert(
            "sentiment_score".to_string(),
            json!(sentiment_score)
        );
    }

    Ok(response)
}

fn calculate_mock_sentiment(text: &str) -> f64 {
    let positive_words = ["good", "great", "excellent", "happy", "wonderful"];
    let negative_words = ["bad", "terrible", "awful", "sad", "horrible"];

    let lowercase_text = text.to_lowercase();
    let words: Vec<&str> = lowercase_text.split_whitespace().collect();
    let mut score: f64 = 0.0;

    for word in words {
        if positive_words.contains(&word) {
            score += 0.2;
        }
        if negative_words.contains(&word) {
            score -= 0.2;
        }
    }

    score.clamp(-1.0, 1.0)
} 