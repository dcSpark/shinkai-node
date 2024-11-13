use shinkai_message_primitives::schemas::shinkai_tool_offering::{ShinkaiToolOffering, UsageType};
use shinkai_tools_primitives::tools::{
    argument::{ToolArgument, ToolOutputArg},
    shinkai_tool::ShinkaiToolHeader,
};

pub fn get_custom_tools() -> Vec<ShinkaiToolHeader> {
    let mut custom_tools = Vec::new();

    // LLM Tool
    let llm_tool = ShinkaiToolHeader {
        name: "Shinkai LLM Prompt Processor".to_string(),
        toolkit_name: "shinkai_custom".to_string(),
        description: "Generic tool for processing any prompt using an LLM, analyzing the request and returning a string as output".to_string(),
        tool_router_key: "local:::shinkai_custom:::llm_prompt_processor".to_string(),
        tool_type: "Internal".to_string(),
        formatted_tool_summary_for_ui: "Tool for processing prompts with LLM".to_string(),
        author: "Shinkai".to_string(),
        version: "1.0".to_string(),
        enabled: true,
        input_args: vec![
            ToolArgument::new(
                "prompt".to_string(),
                "string".to_string(),
                "The prompt to process".to_string(),
                true,
            ),
        ],
        output_arg: ToolOutputArg {
            json: r#"{"type": "object", "properties": {"message": {"type": "string"}}}"#.to_string(),
        },
        config: None,
        usage_type: None,
        tool_offering: None,
    };
    custom_tools.push(llm_tool);

    // Calculator Tool
    let calculator = ShinkaiToolHeader {
        name: "Shinkai: Calculator".to_string(),
        toolkit_name: "shinkai_custom".to_string(),
        description: "Performs basic arithmetic operations".to_string(),
        tool_router_key: "local:::shinkai_custom:::calculator".to_string(),
        tool_type: "Internal".to_string(),
        formatted_tool_summary_for_ui: "Basic calculator for arithmetic operations".to_string(),
        author: "Shinkai".to_string(),
        version: "1.0".to_string(),
        enabled: true,
        input_args: vec![
            ToolArgument::new(
                "operation".to_string(),
                "string".to_string(),
                "The operation to perform (add, subtract, multiply, divide)".to_string(),
                true,
            ),
            ToolArgument::new("x".to_string(), "number".to_string(), "First number".to_string(), true),
            ToolArgument::new("y".to_string(), "number".to_string(), "Second number".to_string(), true),
        ],
        output_arg: ToolOutputArg {
            json: r#"{"type": "object", "properties": {"result": {"type": "number"}}}"#.to_string(),
        },
        config: None,
        usage_type: None,
        tool_offering: None,
    };
    custom_tools.push(calculator);

    // Text Analyzer Tool
    let text_analyzer = ShinkaiToolHeader {
        name: "Shinkai: Text Analyzer".to_string(),
        toolkit_name: "shinkai_custom".to_string(),
        description: "Analyzes text and provides statistics".to_string(),
        tool_router_key: "local:::shinkai_custom:::text_analyzer".to_string(),
        tool_type: "Internal".to_string(),
        formatted_tool_summary_for_ui: "Tool for analyzing text and providing statistics".to_string(),
        author: "Shinkai".to_string(),
        version: "1.0".to_string(),
        enabled: true,
        input_args: vec![
            ToolArgument::new(
                "text".to_string(),
                "string".to_string(),
                "The text to analyze".to_string(),
                true,
            ),
            ToolArgument::new(
                "include_sentiment".to_string(),
                "boolean".to_string(),
                "Whether to include sentiment analysis".to_string(),
                false,
            ),
        ],
        output_arg: ToolOutputArg {
            json: r#"{"type": "object", "properties": {"word_count": {"type": "integer"}, "character_count": {"type": "integer"}, "sentiment_score": {"type": "number"}}}"#.to_string(),
        },
        config: None,
        usage_type: None,
        tool_offering: None,
    };
    custom_tools.push(text_analyzer);

    custom_tools
}
