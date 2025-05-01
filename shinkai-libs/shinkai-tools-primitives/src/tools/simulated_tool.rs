use super::parameters::Parameters;
use super::tool_config::ToolConfig;
use super::tool_playground::ToolPlaygroundMetadata;
use super::tool_types::{RunnerType, ToolResult};
use crate::tools::error::ToolError;
use serde_json::{json, Map};
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_tools_runner::tools::run_result::RunResult;
use std::collections::HashMap;
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SimulatedTool {
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub config: Vec<ToolConfig>,
    pub input_args: Parameters,
    pub result: ToolResult,
    pub embedding: Option<Vec<f32>>,
}

impl SimulatedTool {
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json)?;
        Ok(deserialized)
    }

    pub fn get_author(&self) -> String {
        "@@localhost.shinkai".to_string()
    }

    pub fn get_version(&self) -> Option<String> {
        None
    }

    pub fn get_source(&self) -> String {
        "@@simulated.local".to_string()
    }

    pub fn get_tool_router_key(&self) -> String {
        let trk = ToolRouterKey {
            source: self.get_source(),
            author: self.get_author(),
            name: self.name.clone(),
            version: None,
        };
        trk.to_string_without_version()
    }

    pub async fn build_example_json(
        key: &String,
        hash_map: &Map<String, serde_json::Value>,
    ) -> Result<(String, serde_json::Value), ToolError> {
        let unknown = serde_json::Value::String("unknown".to_string());
        let r#type = hash_map.get("type").unwrap_or(&unknown).as_str().unwrap_or_default();

        if r#type == "array" {
            let items = hash_map.get("items");
            if items.is_some() {
                let items = items.unwrap().as_object().unwrap();
                let _items_type = items
                    .get("type")
                    .unwrap_or(&unknown.clone())
                    .as_str()
                    .unwrap_or_default();
            }
        }

        match r#type {
            "string" => {
                return Ok((key.to_string(), serde_json::Value::String("EXAMPLE_VALUE".to_string())));
            }
            "number" => {
                return Ok((
                    key.to_string(),
                    serde_json::Value::Number(serde_json::Number::from(100)),
                ));
            }
            "boolean" => {
                return Ok((key.to_string(), serde_json::Value::Bool(true)));
            }
            "array" => {
                return Ok((key.to_string(), serde_json::Value::Array(vec![])));
            }
            "object" => {
                // Create recursive call to build the object
                let properties = hash_map.get("properties");
                if properties.is_none() {
                    return Ok((key.to_string(), json!({})));
                }
                let properties = properties.unwrap().as_object().unwrap();
                let mut object = HashMap::new();
                for (property_key, property_value) in properties {
                    let property_values = property_value.as_object().unwrap();
                    let property_value_json =
                        Box::pin(SimulatedTool::build_example_json(&property_key, property_values)).await?;
                    object.insert(property_key.to_string(), property_value_json);
                }
                let o = serde_json::to_value(&object).unwrap();
                return Ok((key.to_string(), o));
            }
            _ => {
                return Err(ToolError::FailedJSONParsing);
            }
        }
    }

    pub async fn run(
        &self,
        bearer_token: String,
        api_ip: String,
        api_port: u16,
        app_id: String,
        tool_id: String,
        llm_provider: String,
        parameters: serde_json::Map<String, serde_json::Value>,
        extra_config: Vec<ToolConfig>,
    ) -> Result<RunResult, ToolError> {
        let metadata = ToolPlaygroundMetadata {
            name: self.name.clone(),
            homepage: None,
            version: "1.0.0".to_string(),
            description: self.description.clone(),
            author: "@@local.shinkai".to_string(),
            keywords: self.keywords.clone(),
            configurations: self.config.clone(),
            parameters: self.input_args.clone(),
            result: self.result.clone(),
            sql_tables: vec![],
            sql_queries: vec![],
            tools: None,
            oauth: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![],
            tool_set: None,
        };

        let mut example_result: HashMap<String, serde_json::Value> = HashMap::new();
        let properties = self.result.properties.as_object().unwrap();
        println!("result: {:?}", self.result.properties);
        println!("properties: {:?}", properties);
        for (property_name, object) in properties.iter() {
            println!("property_name: {}", property_name);
            println!("object: {:?}", object);
            let property_values = object.as_object().unwrap();
            let (key, value) = SimulatedTool::build_example_json(&property_name, property_values).await?;
            example_result.insert(key, value);
        }

        let prompt = format!(
            r#"
<rules>
  * You are a TOOL simulator.
  * The TOOL description is given in the metadata tag.
  * The TOOL inputs: "parameters" and "configuration" are given in the inputs tag.
  * Given the description, parameters and configuration, generate a mock response.
  * Simulate the expected successful output of the tool.
  * Do not let the user know this is a mock response.
  * use the output_example tag as example for the response.
</rules>

<metadata>
{}
</metadata>

<inputs>
parameters: {}

extra_config: {}
</inputs>

<formatting>
  * Write a valid JSON Object
  * Follow the output_example tag as base example, you may add more key/values.
  * Do not output any other comments, ideas, planning, thoughts or comments.
</formatting>

<output_example>
```json
{}
```
</output_example>
            "#,
            serde_json::to_value(&metadata).unwrap(),
            serde_json::to_value(&parameters).unwrap(),
            serde_json::to_value(&extra_config).unwrap(),
            serde_json::to_value(&example_result).unwrap(),
        );

        // TODO Check if HTTP or HTTPS is used
        let url = format!("http://{}:{}/v2/tool_execution", api_ip, api_port);
        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .header("Authorization", format!("Bearer {}", bearer_token))
            .header("x-shinkai-tool-id", tool_id)
            .header("x-shinkai-app-id", app_id)
            .header("x-shinkai-llm-provider", llm_provider.clone())
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&json!({
                "tool_router_key": "local:::__official_shinkai:::shinkai_llm_prompt_processor",
                "llm_provider": llm_provider.clone(),
                "parameters": {
                    "prompt": prompt
                }
            }))
            .send()
            .await?;

        let body = response.json::<serde_json::Value>().await?;
        // We expect the response to have this format, but cannot guarantee it
        // So we try to parse the response as JSON and if it fails, we return the entire object
        // {
        //  "message": "```json\n{\"random_number\":57}\n```"
        // }
        if body.get("message").is_none() {
            return Err(ToolError::ExecutionError(format!(
                "[SimulatedTool] No message found in response: {}",
                body
            )));
        }

        let message_value = body.get("message").unwrap();
        let message = message_value.as_str().unwrap_or_default();
        let mut message_split = message.split("\n").collect::<Vec<&str>>();
        let len = message_split.clone().len();

        if message_split[0] == "```json" {
            message_split[0] = "";
        }
        if message_split[len - 1] == "```" {
            message_split[len - 1] = "";
        }
        let cleaned_json = message_split.join(" ");

        match serde_json::from_str::<serde_json::Value>(&cleaned_json) {
            Ok(data) => return Ok(RunResult { data }),
            Err(e) => {
                println!(
                    "[SimulatedTool] Could not parse as JSON: {} - so returning entire object",
                    body
                );
                return Ok(RunResult {
                    data: message_value.clone(),
                });
            }
        }
    }
}
