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
        CREATE TABLE IF NOT EXISTS url_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT NOT NULL,
            parse_date DATETIME DEFAULT CURRENT_TIMESTAMP,in
            parse_time_ms INTEGER,
            url_raw_dump TEXT
        );
        
        Example insert:
        INSERT INTO url_metrics (url, parse_time_ms, url_raw_dump) VALUES ('https://example.com', 150, '<li>data</li>');
        
        Example read:
        SELECT * FROM url_metrics WHERE parse_date > datetime('now', '-1 day');
        SELECT url, parse_time_ms FROM url_metrics WHERE parse_time_ms > 100 ORDER BY parse_date DESC LIMIT 10;"#
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
            ),
            ToolArgument::new(
                "path".to_string(),
                "string".to_string(),
                "Path to the SQLite database file".to_string(),
                true,
            ),
        ],
        output_arg: ToolOutputArg {
            json: r#"{"type": "object", "properties": {"result": {"type": "string"}}}"#.to_string(),
        },
        config: None,
        usage_type: None,
        tool_offering: None,
    };
    custom_tools.push(sqlite_tool);

    custom_tools
}
