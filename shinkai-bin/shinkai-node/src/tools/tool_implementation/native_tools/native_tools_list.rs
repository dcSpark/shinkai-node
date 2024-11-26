use shinkai_tools_primitives::tools::rust_tools::RustTool;

use crate::tools::tool_implementation::sql_processor::SQLProcessorTool;

pub struct NativeToolsList {}

impl NativeToolsList {
    pub async fn static_tools() -> Vec<RustTool> {
        let mut tools = Vec::new();

        let sql_tool = RustTool::from_shinkai_tool_header(&SQLProcessorTool::new().tool).unwrap();
        tools.push(sql_tool);

        tools
    }
}
