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

    custom_tools
}
