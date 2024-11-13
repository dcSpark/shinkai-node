use std::collections::HashMap;
use shinkai_tools_runner::{built_in_tools, tools::tool_definition::ToolDefinition};

pub fn get_built_in_tools() -> HashMap<String, ToolDefinition> {
    built_in_tools::get_tools()
        .into_iter()
        .collect::<HashMap<_, _>>()
} 