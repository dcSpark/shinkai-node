use shinkai_tools_primitives::tools::rust_tools::RustTool;

use crate::tools::tool_implementation::llm_prompt_processor::LmPromptProcessorTool;

use super::{sql_processor::SQLProcessorTool, tool_knowledge::KnowledgeTool};

pub struct NativeToolsList {}

impl NativeToolsList {
    pub async fn static_tools() -> Vec<RustTool> {
        let mut tools = Vec::new();

        let sql_tool = RustTool::from_shinkai_tool_header(&SQLProcessorTool::new().tool).unwrap();
        tools.push(sql_tool);

        let llm_tool = RustTool::from_shinkai_tool_header(&LmPromptProcessorTool::new().tool).unwrap();
        tools.push(llm_tool);

        let tool_knowledge = RustTool::from_shinkai_tool_header(&KnowledgeTool::new().tool).unwrap();
        tools.push(tool_knowledge);

        tools
    }
}
