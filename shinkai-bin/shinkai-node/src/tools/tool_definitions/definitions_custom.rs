use shinkai_tools_primitives::tools::{
    argument::{ToolArgument, ToolOutputArg},
    shinkai_tool::ShinkaiToolHeader,
};

// TODO keep in sync with execution_custom.rs
pub fn get_custom_tools() -> Vec<ShinkaiToolHeader> {
    let mut custom_tools = Vec::new();

    // LLM Tool
    let llm_tool = ShinkaiToolHeader {
        name: "Shinkai LLM Prompt Processor".to_string(),
        toolkit_name: "shinkai_custom".to_string(),
        description: "Generic tool for processing any prompt using an LLM, analyzing the request and returning a string as output".to_string(),
        tool_router_key: "local:::rust_toolkit:::shinkai_llm_prompt_processor".to_string(),
        tool_type: "Rust".to_string(),
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

    // SQLite Tool
    let sqlite_tool = ShinkaiToolHeader {
        name: "Shinkai SQLite Query Executor".to_string(),
        toolkit_name: "shinkai_custom".to_string(),
        description: r#"Tool for executing SQLite queries on a specified database file. 
        Table creation should always use 'CREATE TABLE IF NOT EXISTS'.
        
        Example table creation:
        CREATE TABLE IF NOT EXISTS table_name (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            field_1 TEXT NOT NULL,
            field_2 DATETIME DEFAULT CURRENT_TIMESTAMP,
            field_3 INTEGER,
            field_4 TEXT
        );
        
        Example insert:
        INSERT INTO table_name (field_1, field_3, field_4) VALUES ('value_1', 1, 'value_4');
        
        Example read:
        SELECT * FROM table_name WHERE field_2 > datetime('now', '-1 day');
        SELECT field_1, field_3 FROM table_name WHERE field_3 > 100 ORDER BY field_2 DESC LIMIT 10;"#
            .to_string(),
        tool_router_key: "local:::rust_toolkit:::shinkai_sqlite_query_executor".to_string(),
        tool_type: "Rust".to_string(),
        formatted_tool_summary_for_ui: "Execute SQLite queries".to_string(),
        author: "Shinkai".to_string(),
        version: "1.0".to_string(),
        enabled: true,
        input_args: vec![
            ToolArgument::new(
                "query".to_string(),
                "string".to_string(),
                "The SQL query to execute".to_string(),
                true,
            )
        ],
        output_arg: ToolOutputArg {
            json: r#"{"type": "object", "properties": {"result": {"oneOf": [{"type": "string"},{"type": "array"}]}, "type": {"type": "string"}, "rowCount": {"type": "number"}, "rowsAffected": {"type": "number"}}}"#.to_string(),
        },
        config: None,
        usage_type: None,
        tool_offering: None,
    };
    custom_tools.push(sqlite_tool);

    custom_tools
}
