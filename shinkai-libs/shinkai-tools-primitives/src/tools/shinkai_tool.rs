use std::env;

use crate::tools::error::ToolError;
use crate::tools::rust_tools::RustTool;
use serde_json::{self, Value};

use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_message_primitives::schemas::{
    indexable_version::IndexableVersion,
    shinkai_tool_offering::{ShinkaiToolOffering, UsageType},
};

use super::tool_config::OAuth;
use super::tool_playground::{SqlQuery, SqlTable};
use super::{
    deno_tools::DenoTool, network_tool::NetworkTool, parameters::Parameters, python_tools::PythonTool,
    tool_config::ToolConfig, tool_output_arg::ToolOutputArg,
};

pub type IsEnabled = bool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "content")]
pub enum ShinkaiTool {
    Rust(RustTool, IsEnabled),
    Network(NetworkTool, IsEnabled),
    Deno(DenoTool, IsEnabled),
    Python(PythonTool, IsEnabled),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Assets {
    pub file_name: String,
    pub data: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShinkaiToolWithAssets {
    pub tool: ShinkaiTool,
    pub assets: Option<Vec<Assets>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ShinkaiToolHeader {
    pub name: String,
    pub description: String,
    pub tool_router_key: String,
    pub tool_type: String,
    pub formatted_tool_summary_for_ui: String,
    pub author: String,
    pub version: String,
    pub enabled: bool,
    pub input_args: Parameters,
    pub output_arg: ToolOutputArg,
    pub config: Option<Vec<ToolConfig>>,
    pub usage_type: Option<UsageType>, // includes pricing
    // Note: do we need usage_type? it's already contained in the tool_offering
    pub tool_offering: Option<ShinkaiToolOffering>,
}

impl ShinkaiToolHeader {
    /// Sanitize the config by removing key-values from BasicConfig
    pub fn sanitize_config(&mut self) {
        if let Some(configs) = &self.config {
            self.config = Some(configs.iter().map(|config| config.sanitize()).collect());
        }
    }
}

impl ShinkaiTool {
    /// Generate a ShinkaiToolHeader from a ShinkaiTool
    pub fn to_header(&self) -> ShinkaiToolHeader {
        ShinkaiToolHeader {
            name: self.name(),
            description: self.description(),
            tool_router_key: self.tool_router_key().to_string_without_version(),
            tool_type: self.tool_type().to_string(),
            formatted_tool_summary_for_ui: self.formatted_tool_summary_for_ui(),
            author: self.author(),
            version: self.version(),
            enabled: self.is_enabled(),
            input_args: self.input_args(),
            output_arg: self.output_arg(),
            config: self.get_js_tool_config().cloned(),
            usage_type: self.get_usage_type(),
            tool_offering: None,
        }
    }

    /// The key that this tool will be stored under in the tool router
    pub fn tool_router_key(&self) -> ToolRouterKey {
        let (provider, author, name) = match self {
            ShinkaiTool::Rust(r, _) => ("local".to_string(), r.author(), r.name.clone()),
            ShinkaiTool::Network(n, _) => (n.provider.to_string(), n.author.to_string(), n.name.clone()),
            ShinkaiTool::Deno(d, _) => ("local".to_string(), d.author.clone(), d.name.clone()),
            ShinkaiTool::Python(p, _) => ("local".to_string(), p.author.clone(), p.name.clone()),
            _ => unreachable!(),
        };
        ToolRouterKey::new(provider, author, name, None)
    }

    /// Sanitize the config by removing key-values from BasicConfig
    pub fn sanitize_config(&mut self) {
        match self {
            ShinkaiTool::Deno(d, _) => {
                d.config = d.config.clone().iter().map(|config| config.sanitize()).collect();
            }
            ShinkaiTool::Python(p, _) => {
                p.config = p.config.clone().iter().map(|config| config.sanitize()).collect();
            }
            _ => (),
        }
    }

    /// Generate the key that this tool will be stored under in the tool router
    pub fn gen_router_key(source: String, author: String, name: String) -> String {
        let tool_router_key = ToolRouterKey::new(source, author, name, None);
        tool_router_key.to_string_without_version()
    }

    /// Tool name
    pub fn name(&self) -> String {
        match self {
            ShinkaiTool::Rust(r, _) => r.name.clone(),
            ShinkaiTool::Network(n, _) => n.name.clone(),
            ShinkaiTool::Deno(d, _) => d.name.clone(),
            ShinkaiTool::Python(p, _) => p.name.clone(),
        }
    }
    /// Tool description
    pub fn description(&self) -> String {
        match self {
            ShinkaiTool::Rust(r, _) => r.description.clone(),
            ShinkaiTool::Network(n, _) => n.description.clone(),
            ShinkaiTool::Deno(d, _) => d.description.clone(),
            ShinkaiTool::Python(p, _) => p.description.clone(),
        }
    }

    /// Returns the input arguments of the tool
    pub fn input_args(&self) -> Parameters {
        match self {
            ShinkaiTool::Rust(r, _) => r.input_args.clone(),
            ShinkaiTool::Network(n, _) => n.input_args.clone(),
            ShinkaiTool::Deno(d, _) => d.input_args.clone(),
            ShinkaiTool::Python(p, _) => p.input_args.clone(),
        }
    }

    /// Returns the input arguments of the tool
    pub fn output_arg(&self) -> ToolOutputArg {
        match self {
            ShinkaiTool::Rust(r, _) => r.output_arg.clone(),
            ShinkaiTool::Network(n, _) => n.output_arg.clone(),
            ShinkaiTool::Deno(d, _) => d.output_arg.clone(),
            ShinkaiTool::Python(p, _) => p.output_arg.clone(),
        }
    }

    /// Returns the output arguments of the tool
    pub fn tool_type(&self) -> &'static str {
        match self {
            ShinkaiTool::Rust(_, _) => "Rust",
            ShinkaiTool::Network(_, _) => "Network",
            ShinkaiTool::Deno(_, _) => "Deno",
            ShinkaiTool::Python(_, _) => "Python",
        }
    }

    /// Returns the SQL queries of the tool
    pub fn sql_queries(&self) -> Vec<SqlQuery> {
        match self {
            ShinkaiTool::Deno(d, _) => d.sql_queries.clone().unwrap_or_default(),
            ShinkaiTool::Python(p, _) => p.sql_queries.clone().unwrap_or_default(),
            _ => vec![],
        }
    }

    /// Returns the SQL tables of the tool
    pub fn sql_tables(&self) -> Vec<SqlTable> {
        match self {
            ShinkaiTool::Deno(d, _) => d.sql_tables.clone().unwrap_or_default(),
            ShinkaiTool::Python(p, _) => p.sql_tables.clone().unwrap_or_default(),
            _ => vec![],
        }
    }

    pub fn get_oauth(&self) -> Option<Vec<OAuth>> {
        match self {
            ShinkaiTool::Deno(d, _) => d.oauth.clone(),
            ShinkaiTool::Python(p, _) => p.oauth.clone(),
            _ => None,
        }
    }

    pub fn get_tools(&self) -> Vec<ToolRouterKey> {
        match self {
            ShinkaiTool::Deno(d, _) => d.tools.clone(),
            ShinkaiTool::Python(p, _) => p.tools.clone(),
            _ => vec![],
        }
    }

    pub fn get_assets(&self) -> Option<Vec<String>> {
        match self {
            ShinkaiTool::Deno(d, _) => d.assets.clone(),
            ShinkaiTool::Python(p, _) => p.assets.clone(),
            _ => None,
        }
    }

    pub fn get_homepage(&self) -> Option<String> {
        match self {
            ShinkaiTool::Deno(d, _) => d.homepage.clone(),
            ShinkaiTool::Python(p, _) => p.homepage.clone(),
            _ => None,
        }
    }

    /// Returns a formatted summary of the tool
    pub fn formatted_tool_summary_for_ui(&self) -> String {
        format!(
            "Tool Name: {}\nAuthor: {}\nDescription: {}",
            self.name(),
            self.author(),
            self.description(),
        )
    }

    /// Sets the embedding for the tool
    pub fn set_embedding(&mut self, embedding: Vec<f32>) {
        match self {
            ShinkaiTool::Rust(r, _) => r.tool_embedding = Some(embedding),
            ShinkaiTool::Network(n, _) => n.embedding = Some(embedding),
            ShinkaiTool::Deno(d, _) => d.embedding = Some(embedding),
            ShinkaiTool::Python(p, _) => p.embedding = Some(embedding),
        }
    }

    /// Returns the tool formatted as a JSON object for the function call format
    pub fn json_function_call_format(&self) -> Result<serde_json::Value, ToolError> {
        // Get the ToolRouterKey instance
        let tool_router_key = self.tool_router_key();

        // Extract the tool name directly from the ToolRouterKey
        let tool_name = tool_router_key.name.clone();

        let summary = serde_json::json!({
            "type": "function",
            "function": {
                "name": tool_name,
                "description": self.description(),
                "tool_router_key": tool_router_key.to_string_without_version(),
                "parameters": self.input_args()
            },
        });

        Ok(summary)
    }

    pub fn json_string_function_call_format(&self) -> Result<String, ToolError> {
        let summary_value = self.json_function_call_format()?;
        serde_json::to_string(&summary_value).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Formats the tool's info into a String to be used for generating the tool's embedding.
    pub fn format_embedding_string(&self) -> String {
        let formatted_name = self.name().replace("shinkai__", "").replace('_', " ");
        format!("{} {}", formatted_name, self.description())
    }

    /// Returns the embedding if it exists
    pub fn get_embedding(&self) -> Option<Vec<f32>> {
        match self {
            ShinkaiTool::Rust(r, _) => r.tool_embedding.clone(),
            ShinkaiTool::Network(n, _) => n.embedding.clone(),
            ShinkaiTool::Deno(d, _) => d.embedding.clone(),
            ShinkaiTool::Python(p, _) => p.embedding.clone(),
        }
    }

    /// Returns an Option<ToolConfig> based on an environment variable
    pub fn get_config_from_env(&self) -> Option<ToolConfig> {
        // Get the ToolRouterKey instance and convert it to a string
        let tool_key = self.tool_router_key().to_string_without_version().replace(":::", "___");
        let env_var_key = format!("TOOLKIT_{}", tool_key);

        if let Ok(env_value) = env::var(env_var_key) {
            // Attempt to parse the environment variable as JSON
            if let Ok(value) = serde_json::from_str::<Value>(&env_value) {
                // Attempt to deserialize the JSON value into a ToolConfig
                return ToolConfig::from_value(&value);
            }
        }

        None
    }

    /// Returns the author of the tool
    pub fn author(&self) -> String {
        match self {
            ShinkaiTool::Rust(r, _) => r.author(),
            ShinkaiTool::Network(n, _) => n.author.clone(),
            ShinkaiTool::Deno(d, _) => d.author.clone(),
            ShinkaiTool::Python(p, _) => p.author.clone(),
        }
    }

    /// Returns the version of the tool
    pub fn version(&self) -> String {
        match self {
            ShinkaiTool::Rust(r, _) => "1.0.0".to_string(),
            ShinkaiTool::Network(n, _) => n.version.clone(),
            ShinkaiTool::Deno(d, _) => d.version.clone(),
            ShinkaiTool::Python(p, _) => p.version.clone(),
        }
    }

    /// Get the usage type, only valid for NetworkTool
    pub fn get_usage_type(&self) -> Option<UsageType> {
        if let ShinkaiTool::Network(n, _) = self {
            Some(n.usage_type.clone())
        } else {
            None
        }
    }

    /// Check if the tool is enabled
    pub fn is_enabled(&self) -> bool {
        match self {
            ShinkaiTool::Rust(_, enabled) => *enabled,
            ShinkaiTool::Network(_, enabled) => *enabled,
            ShinkaiTool::Deno(_, enabled) => *enabled,
            ShinkaiTool::Python(_, enabled) => *enabled,
        }
    }

    /// Enable the tool
    pub fn enable(&mut self) {
        match self {
            ShinkaiTool::Rust(_, enabled) => *enabled = true,
            ShinkaiTool::Network(_, enabled) => *enabled = true,
            ShinkaiTool::Deno(_, enabled) => *enabled = true,
            ShinkaiTool::Python(_, enabled) => *enabled = true,
        }
    }

    /// Disable the tool
    pub fn disable(&mut self) {
        match self {
            ShinkaiTool::Rust(_, enabled) => *enabled = false,
            ShinkaiTool::Network(_, enabled) => *enabled = false,
            ShinkaiTool::Deno(_, enabled) => *enabled = false,
            ShinkaiTool::Python(_, enabled) => *enabled = false,
        }
    }

    /// Get the config from a JSTool, return None if it's another type
    pub fn get_js_tool_config(&self) -> Option<&Vec<ToolConfig>> {
        if let ShinkaiTool::Deno(js_tool, _) = self {
            Some(&js_tool.config)
        } else {
            None
        }
    }

    pub fn get_config(&self) -> Vec<ToolConfig> {
        match self {
            ShinkaiTool::Rust(_, _) => vec![],
            ShinkaiTool::Network(_, _) => vec![],
            ShinkaiTool::Deno(js_tool, _) => js_tool.config.clone(),
            ShinkaiTool::Python(python_tool, _) => python_tool.config.clone(),
        }
    }

    /// Check if the tool can be enabled
    pub fn can_be_enabled(&self) -> bool {
        match self {
            ShinkaiTool::Rust(_, _) => true,
            ShinkaiTool::Network(n_tool, _) => n_tool.check_required_config_fields(),
            ShinkaiTool::Deno(deno_tool, _) => deno_tool.check_required_config_fields(),
            ShinkaiTool::Python(_, _) => true,
        }
    }

    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Convert from json
    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json).map_err(|e| ToolError::ParseError(e.to_string()))?;
        Ok(deserialized)
    }

    /// Check if the tool is Rust-based
    pub fn is_rust_based(&self) -> bool {
        matches!(self, ShinkaiTool::Rust(_, _))
    }

    /// Check if the tool is JS-based
    pub fn is_js_based(&self) -> bool {
        matches!(self, ShinkaiTool::Deno(_, _))
    }

    /// Check if the tool is Workflow-based
    pub fn is_network_based(&self) -> bool {
        matches!(self, ShinkaiTool::Network(_, _))
    }

    pub fn version_indexable(&self) -> Result<IndexableVersion, String> {
        IndexableVersion::from_string(&self.version())
    }

    /// Returns the version number using IndexableVersion
    pub fn version_number(&self) -> Result<u64, String> {
        let indexable_version = self.version_indexable()?;
        Ok(indexable_version.get_version_number())
    }

    /// Returns a sanitized version of the tool name where all characters are lowercase
    /// and any non-alphanumeric characters (except '-' and '_') are replaced with underscores
    pub fn internal_sanitized_name(&self) -> String {
        self.name()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect::<String>()
    }
}

impl From<RustTool> for ShinkaiTool {
    fn from(tool: RustTool) -> Self {
        ShinkaiTool::Rust(tool, true)
    }
}

impl From<DenoTool> for ShinkaiTool {
    fn from(tool: DenoTool) -> Self {
        ShinkaiTool::Deno(tool, true)
    }
}

impl From<NetworkTool> for ShinkaiTool {
    fn from(tool: NetworkTool) -> Self {
        ShinkaiTool::Network(tool, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::deno_tools::{DenoTool, ToolResult};
    use serde_json::json;
    use shinkai_tools_runner::tools::tool_definition::ToolDefinition;

    #[test]
    fn test_gen_router_key() {
        // Create a mock DenoTool with all required fields
        let deno_tool = DenoTool {
            name: "Shinkai: Download Pages".to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            description: "Downloads one or more URLs and converts their HTML content to Markdown".to_string(),
            input_args: Parameters::new(),
            output_arg: ToolOutputArg { json: "".to_string() },
            config: vec![],
            author: "@@official.shinkai".to_string(),
            version: "1.0.0".to_string(),
            js_code: "".to_string(),
            tools: vec![],
            keywords: vec![],
            activated: false,
            embedding: None,
            result: ToolResult::new(
                "object".to_string(),
                json!({
                    "markdowns": { "type": "array", "items": { "type": "string" } }
                }),
                vec!["markdowns".to_string()],
            ),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
        };

        // Create a ShinkaiTool instance
        let shinkai_tool = ShinkaiTool::Deno(deno_tool, false);

        // Generate the router key
        let router_key = shinkai_tool.tool_router_key();

        // Expected pattern: [^a-z0-9_]+ (plus the :::)
        let expected_key = "local:::__official_shinkai:::shinkai__download_pages";

        // Assert that the generated key matches the expected pattern
        assert_eq!(router_key.to_string_without_version(), expected_key);
    }

    #[test]
    fn test_set_playground_tool() {
        let tool_definition = ToolDefinition {
            id: "shinkai-tool-download-website".to_string(),
            name: "Download Website".to_string(),
            description: "Downloads a website and converts its content into Markdown.".to_string(),
            configurations: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch"
                    }
                },
                "required": ["url"]
            }),
            result: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            author: "@@my_local_ai.sep-shinkai".to_string(),
            keywords: vec![
                "Deno".to_string(),
                "Markdown".to_string(),
                "HTML to Markdown".to_string(),
            ],
            code: Some("import { getHomePath } from './shinkai-local-support.ts';\n\n...".to_string()), // Truncated for brevity
            embedding_metadata: None,
        };

        let input_args = Parameters::with_single_property("url", "string", "The URL to fetch", true);

        let deno_tool = DenoTool {
            name: "shinkai__download_website".to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            version: "1.0.0".to_string(),
            description: tool_definition.description.clone(),
            input_args: input_args.clone(),
            output_arg: ToolOutputArg {
                json: tool_definition.result.to_string(),
            },
            config: vec![],
            author: tool_definition.author.clone(),
            js_code: tool_definition.code.clone().unwrap_or_default(),
            tools: vec![],
            keywords: tool_definition.keywords.clone(),
            activated: false,
            embedding: None,
            result: ToolResult::new(
                "object".to_string(),
                tool_definition.result["properties"].clone(),
                vec![],
            ),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
        };

        let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);
        eprintln!("shinkai_tool: {:?}", shinkai_tool);

        eprintln!("shinkai params: {:?}", shinkai_tool.input_args());

        assert_eq!(shinkai_tool.name(), "shinkai__download_website");
        assert_eq!(
            shinkai_tool.description(),
            "Downloads a website and converts its content into Markdown."
        );
        assert_eq!(shinkai_tool.tool_type(), "Deno");
        assert!(shinkai_tool.is_enabled());
    }

    #[test]
    fn test_deserialize_shinkai_tool() {
        let json_payload = r#"
        {
            "type": "Deno",
            "content": [
                {
                    "description": "Tool for getting the default address of a Coinbase wallet",
                    "version": "1.0.0",
                    "activated": false,
                    "assets": null,
                    "author": "Shinkai",
                    "file_inbox": null,
                    "toolkit_name": "shinkai-tool-coinbase-get-my-address",
                    "sql_tables": [],
                    "sql_queries": [],
                    "embedding": [],
                    "oauth": null,
                    "config": [],
                    "keywords": [
                        "coinbase",
                        "address",
                        "shinkai"
                    ],
                    "tools": [],
                    "result": {
                        "type": "object",
                        "properties": {
                            "address": {
                                "type": "string",
                                "description": "hey"
                            }
                        },
                        "required": [
                            "address"
                        ]
                    },
                    "input_args": {
                        "type": "object",
                        "properties": {
                            "walletId": {
                                "type": "string",
                                "nullable": true,
                                "description": "The ID of the wallet to get the address for"
                            }
                        },
                        "required": []
                    },
                    "output_arg": {
                        "json": ""
                    },
                    "name": "Shinkai: Coinbase My Address Getter",
                    "js_code": "import { Coinbase, CoinbaseOptions } from 'npm:@coinbase/coinbase-sdk@0.0.16';\\n\\ntype Configurations = {\\n name: string;\\n privateKey: string;\\n walletId?: string;\\n useServerSigner?: string;\\n};\\ntype Parameters = {\\n walletId?: string;\\n};\\ntype Result = {\\n address: string;\\n};\\nexport type Run<C extends Record<string, any>, I extends Record<string, any>, R extends Record<string, any>> = (config: C, inputs: I) => Promise<R>;\\n\\nexport const run: Run<Configurations, Parameters, Result> = async (\\n configurations: Configurations,\\n params: Parameters,\\n): Promise<Result> => {\\n const coinbaseOptions: CoinbaseOptions = {\\n apiKeyName: configurations.name,\\n privateKey: configurations.privateKey,\\n useServerSigner: configurations.useServerSigner === 'true',\\n };\\n const coinbase = new Coinbase(coinbaseOptions);\\n const user = await coinbase.getDefaultUser();\\n\\n // Prioritize walletId from Params over Config\\n const walletId = params.walletId || configurations.walletId;\\n\\n // Throw an error if walletId is not defined\\n if (!walletId) {\\n throw new Error('walletId must be defined in either params or config');\\n }\\n\\n const wallet = await user.getWallet(walletId);\\n console.log(`Wallet retrieved: `, wallet.toString());\\n\\n // Retrieve the list of balances for the wallet\\n const address = await wallet.getDefaultAddress();\\n console.log(`Default Address: `, address);\\n\\n return {\\n address: address?.getId() || '',\\n };\\n};"
                },
                false
            ]
        }
        "#;

        let deserialized_tool: Result<ShinkaiTool, _> = serde_json::from_str(json_payload);
        eprintln!("deserialized_tool: {:?}", deserialized_tool);

        assert!(deserialized_tool.is_ok(), "Failed to deserialize ShinkaiTool");
    }
}
