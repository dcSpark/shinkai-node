use std::collections::HashMap;
use shinkai_tools_runner::tools::tool_definition::{ToolDefinition, EmbeddingMetadata};

pub fn get_custom_tools() -> HashMap<String, ToolDefinition> {
    let mut custom_tools = HashMap::new();
    
    // Example 1: Custom Calculator Tool
    let calculator = ToolDefinition {
        id: "shinkai-tool-calculator".to_string(),
        name: "Shinkai: Calculator".to_string(),
        description: "Performs basic arithmetic operations".to_string(),
        author: "Shinkai".to_string(),
        keywords: vec![
            "math".to_string(),
            "calculator".to_string(),
            "arithmetic".to_string(),
            "basic operations".to_string()
        ],
        configurations: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "description": "The operation to perform (add, subtract, multiply, divide)"
                },
                "x": {
                    "type": "number",
                    "description": "First number"
                },
                "y": {
                    "type": "number",
                    "description": "Second number"
                }
            },
            "required": ["operation", "x", "y"]
        }),
        result: serde_json::json!({
            "type": "object",
            "properties": {
                "result": {
                    "type": "number",
                    "description": "The calculation result"
                }
            },
            "required": ["result"]
        }),
        code: Some("// Calculator implementation in JavaScript/TypeScript\nconst run = async (_configurations, parameters) => {\n    const { operation, x, y } = parameters;\n    let result;\n    switch(operation) {\n        case 'add': result = x + y; break;\n        case 'subtract': result = x - y; break;\n        case 'multiply': result = x * y; break;\n        case 'divide': result = x / y; break;\n        default: throw new Error('Invalid operation');\n    }\n    return { result };\n};".to_string()),
        embedding_metadata: Some(EmbeddingMetadata {
            model_name: "snowflake-arctic-embed:xs".to_string(),
            embeddings: vec![0.0; 10] // Example embeddings
        }),
    };
    custom_tools.insert("calculator".to_string(), calculator);

    // Example 2: Text Analysis Tool
    let text_analyzer = ToolDefinition {
        id: "shinkai-tool-text-analyzer".to_string(),
        name: "Shinkai: Text Analyzer".to_string(),
        description: "Analyzes text and provides statistics".to_string(),
        author: "Shinkai".to_string(),
        keywords: vec![
            "text analysis".to_string(),
            "statistics".to_string(),
            "sentiment analysis".to_string(),
            "text processing".to_string()
        ],
        configurations: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "The text to analyze"
                },
                "include_sentiment": {
                    "type": "boolean",
                    "description": "Whether to include sentiment analysis"
                }
            },
            "required": ["text"]
        }),
        result: serde_json::json!({
            "type": "object",
            "properties": {
                "word_count": {
                    "type": "integer",
                    "description": "Number of words in the text"
                },
                "character_count": {
                    "type": "integer",
                    "description": "Number of characters in the text"
                },
                "sentiment_score": {
                    "type": "number",
                    "description": "Sentiment score (-1 to 1) if requested"
                }
            },
            "required": ["word_count", "character_count"]
        }),
        code: Some("// Text analyzer implementation in JavaScript/TypeScript\nconst run = async (_configurations, parameters) => {\n    const { text, include_sentiment } = parameters;\n    const result = {\n        word_count: text.split(/\\s+/).length,\n        character_count: text.length,\n    };\n    if (include_sentiment) {\n        result.sentiment_score = calculateSentiment(text);\n    }\n    return result;\n};".to_string()),
        embedding_metadata: Some(EmbeddingMetadata {
            model_name: "snowflake-arctic-embed:xs".to_string(),
            embeddings: vec![0.0; 10] // Example embeddings
        }),
    };
    custom_tools.insert("text_analyzer".to_string(), text_analyzer);

    custom_tools
} 