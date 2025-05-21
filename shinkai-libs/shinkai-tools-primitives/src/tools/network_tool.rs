use shinkai_message_primitives::schemas::{shinkai_name::ShinkaiName, shinkai_tool_offering::UsageType};

use super::{
    error::ToolError, parameters::Parameters, shinkai_tool::ShinkaiTool, tool_config::ToolConfig,
    tool_output_arg::ToolOutputArg,
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NetworkTool {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub mcp_enabled: Option<bool>,
    pub provider: ShinkaiName,
    pub activated: bool,
    pub config: Vec<ToolConfig>,
    pub input_args: Parameters,
    pub output_arg: ToolOutputArg,
    pub embedding: Option<Vec<f32>>,
    pub restrictions: Option<String>, // Could be a JSON string or a more structured type
                                      // ^ What was this for? I think it was *internal* user restrictions (e.g. max_requests_per_day, max_total_budget etc.)
    pub payment_url: Option<String>,
    pub facilitator_url: Option<String>,
}
// Asking Myself (AM): do we want transparency about knowing if it's a wrapped JSTool or Workflow?
// TODO: add the same JS configuration to NetworkTool most likely we will use JSTool and Workflows (which is a subgroup)

impl NetworkTool {
    pub fn new(
        name: String,
        description: String,
        version: String,
        author: String,
        provider: ShinkaiName,
        activated: bool,
        config: Vec<ToolConfig>,
        input_args: Parameters,
        output_arg: ToolOutputArg,
        embedding: Option<Vec<f32>>,
        restrictions: Option<String>,
        payment_url: Option<String>,
        facilitator_url: Option<String>,
    ) -> Self {
        Self {
            name,
            description,
            version,
            author,
            provider,
            activated,
            config,
            input_args,
            output_arg,
            embedding,
            restrictions,
            mcp_enabled: Some(false),
        }
    }

    /// Check if all required config fields are set
    pub fn check_required_config_fields(&self) -> bool {
        for config in &self.config {
            let ToolConfig::BasicConfig(basic_config) = config;
            if basic_config.required && basic_config.key_value.is_none() {
                return false;
            }
        }
        true
    }

    /// Convert to JSON string
    pub fn to_json_string(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|e| ToolError::SerializationError(e.to_string()))
    }

    /// The key that this tool will be stored under in the tool router
    pub fn tool_router_key(&self) -> String {
        let shinkai_tool: ShinkaiTool = self.clone().into();
        shinkai_tool.tool_router_key().to_string_without_version()
    }
}
