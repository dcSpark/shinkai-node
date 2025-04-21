use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    error::ToolError,
    parameters::Parameters,
    shinkai_tool::ShinkaiTool,
    tool_config::{OAuth, ToolConfig},
    tool_output_arg::ToolOutputArg,
    tool_playground::{SqlQuery, SqlTable},
    tool_types::{OperatingSystem, RunnerType, ToolResult},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WasmTool {
    pub name: String,
    pub homepage: Option<String>,
    pub author: String,
    pub version: String,
    pub mcp_enabled: Option<bool>,
    pub wasm_code: String,
    #[serde(default)]
    #[serde(deserialize_with = "ToolRouterKey::deserialize_tool_router_keys")]
    #[serde(serialize_with = "ToolRouterKey::serialize_tool_router_keys")]
    pub tools: Vec<ToolRouterKey>,
    pub config: Vec<ToolConfig>,
    pub description: String,
    pub keywords: Vec<String>,
    pub input_args: Parameters,
    pub output_arg: ToolOutputArg,
    pub activated: bool,
    pub embedding: Option<Vec<f32>>,
    pub result: ToolResult,
    pub sql_tables: Option<Vec<SqlTable>>,
    pub sql_queries: Option<Vec<SqlQuery>>,
    pub file_inbox: Option<String>,
    pub oauth: Option<Vec<OAuth>>,
    pub assets: Option<Vec<String>>,
    pub runner: RunnerType,
    pub operating_system: Vec<OperatingSystem>,
    pub tool_set: Option<String>,
    pub tee_config: Option<TeeConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeeConfig {
    pub tee_type: TeeType,
    pub attestation_required: bool,
    pub memory_size: Option<u64>,
    pub cpu_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TeeType {
    IntelSGX,
    AMDSEV,
    AppleT2,
}

impl WasmTool {
    pub fn new(
        name: String,
        author: String,
        version: String,
        wasm_code: String,
        description: String,
        input_args: Parameters,
        output_arg: ToolOutputArg,
        result: ToolResult,
    ) -> Self {
        Self {
            name,
            homepage: None,
            author,
            version,
            mcp_enabled: Some(false),
            wasm_code,
            tools: vec![],
            config: vec![],
            description,
            keywords: vec![],
            input_args,
            output_arg,
            activated: true,
            embedding: None,
            result,
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Linux, OperatingSystem::MacOS],
            tool_set: None,
            tee_config: None,
        }
    }

    pub async fn check(
        &self,
        api_host: String,
        api_port: u16,
        support_files: std::collections::HashMap<String, String>,
        node_storage_path: String,
        app_id: String,
        tool_id: String,
    ) -> Result<Vec<String>, ToolError> {
        // TODO: Implement WASM code validation
        Ok(vec![])
    }

    pub async fn run(
        &self,
        env: std::collections::HashMap<String, String>,
        api_host: String,
        api_port: u16,
        support_files: std::collections::HashMap<String, String>,
        parameters: serde_json::Map<String, Value>,
        extra_config: Vec<ToolConfig>,
        node_storage_path: String,
        app_id: String,
        tool_id: String,
        node_name: shinkai_message_primitives::schemas::shinkai_name::ShinkaiName,
        is_playground: bool,
        tool_router_key: Option<String>,
        mounts: Option<Vec<String>>,
    ) -> Result<Value, ToolError> {
        // TODO: Implement WASM code execution in TEE
        Err(ToolError::ExecutionError("WASM execution not implemented yet".to_string()))
    }
}

impl From<WasmTool> for ShinkaiTool {
    fn from(tool: WasmTool) -> Self {
        ShinkaiTool::Wasm(tool, true)
    }
} 