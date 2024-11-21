use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiToolHeader;

use crate::tools::tool_implementation;

// TODO keep in sync with execution_custom.rs
pub fn get_custom_tools() -> Vec<ShinkaiToolHeader> {
    let mut custom_tools = Vec::new();
    custom_tools.push(tool_implementation::llm_prompt_processor::LmPromptProcessorTool::new().tool);
    custom_tools.push(tool_implementation::sql_processor::SQLProcessorTool::new().tool);
    custom_tools
}
