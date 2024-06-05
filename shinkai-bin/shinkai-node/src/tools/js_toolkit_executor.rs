use crate::tools::error::ToolError;
use crate::tools::js_toolkit::JSToolkit;
use lazy_static::lazy_static;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use std::fs::File;
use std::io;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

lazy_static! {
    pub static ref DEFAULT_LOCAL_TOOLKIT_EXECUTOR_PORT: &'static str = "3000";
}

/// The resulting data from execution a JS tool
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    pub tool: String,
    pub result: Vec<ExecutionResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub name: String,
    #[serde(rename = "type")]
    pub result_type: String,
    pub description: String,
    #[serde(rename = "isOptional")]
    pub is_optional: bool,
    #[serde(rename = "wrapperType")]
    pub wrapper_type: String,
    pub ebnf: String,
    #[serde(rename = "result")]
    pub output: JsonValue,
}

pub enum JSToolkitExecutor {
    Local(JSToolkitExecutorProcess),
    Remote(RemoteJSToolkitExecutor),
}

impl JSToolkitExecutor {
    /// Starts the JS Toolkit Executor locally at default path `./files/shinkai-toolkit-executor.js`
    /// Primarily intended for local testing (executor should be sandboxed for production)
    pub async fn new_local() -> Result<Self, ToolError> {
        let executor = JSToolkitExecutor::new_local_custom_path("./files/shinkai-toolkit-executor.js").await?;
        Ok(executor)
    }

    /// Starts the JS Toolkit Executor locally at a custom path
    /// Primarily intended for local testing (executor should be sandboxed for production)
    pub async fn new_local_custom_path(executor_file_path: &str) -> Result<Self, ToolError> {
        let executor = JSToolkitExecutorProcess::start(executor_file_path)
            .map_err(|_| ToolError::JSToolkitExecutorFailedStarting)?;
        executor.submit_health_check().await?;
        Ok(executor)
    }

    /// Establishes connection to a remotely ran JS Toolkit Executor
    pub async fn new_remote(address: String) -> Result<Self, ToolError> {
        let executor = JSToolkitExecutor::Remote(RemoteJSToolkitExecutor { address });
        executor.submit_health_check().await?;
        Ok(executor)
    }

    /// Submits a health check request to /health_check and checks the response
    pub async fn submit_health_check(&self) -> Result<(), ToolError> {
        let response = self.submit_get_request("/health_check").await?;
        if response.get("status").is_some() {
            Ok(())
        } else {
            Err(ToolError::JSToolkitExecutorNotAvailable)
        }
    }

    /// Submits a toolkit json request to the JS Toolkit Executor
    /// and parses the response into a JSToolkit struct
    pub async fn submit_toolkit_json_request(&self, toolkit_js_code: &str) -> Result<JSToolkit, ToolError> {
        let input_data_json = serde_json::json!({ "source": toolkit_js_code });
        let response = self
            .submit_post_request("/toolkit_json", &input_data_json, &JsonValue::Null)
            .await?;
        JSToolkit::from_toolkit_json(&response, toolkit_js_code)
    }

    /// Submits a headers validation request to the JS Toolkit Executor.
    /// If header validation is successful returns `Ok(())`, else returns error with reason.
    pub async fn submit_headers_validation_request(
        &self,
        toolkit_js_code: &str,
        header_values: &JsonValue,
    ) -> Result<(), ToolError> {
        let input_data_json = serde_json::json!({ "source": toolkit_js_code });
        let response = self
            .submit_post_request("/validate_headers", &input_data_json, header_values)
            .await?;
        if let Some(JsonValue::Bool(result)) = response.get("result") {
            if *result {
                return Ok(());
            }
        } else if let Some(JsonValue::Object(result)) = response.get("result") {
            if let Some(JsonValue::String(error)) = result.get("error") {
                return Err(ToolError::JSToolkitHeaderValidationFailed(error.clone()));
            }
        }
        Err(ToolError::JSToolkitHeaderValidationFailed(
            "Not all required headers provided".to_string(),
        ))
    }

    // Submits a tool execution request to the JS Toolkit Executor
    pub async fn submit_tool_execution_request(
        &self,
        tool_name: &str,
        input_data: &JsonValue,
        toolkit_js_code: &str,
        header_values: &JsonValue,
    ) -> Result<ToolExecutionResult, ToolError> {
        let input_data_json = serde_json::json!({
            "tool": tool_name,
            "input": input_data,
            "source": toolkit_js_code
        });
        let response = self
            .submit_post_request("/execute_tool", &input_data_json, header_values)
            .await?;
        let tool_execution_result: ToolExecutionResult = serde_json::from_value(response)?;
        Ok(tool_execution_result)
    }

    // Submits a get request to the JS Toolkit Executor
    async fn submit_get_request(&self, endpoint: &str) -> Result<JsonValue, ToolError> {
        let client = reqwest::Client::new();
        let address = match self {
            JSToolkitExecutor::Local(process) => &process.address,
            JSToolkitExecutor::Remote(remote) => &remote.address,
        };

        let response = client.get(&format!("{}{}", address, endpoint)).send().await?;

        Ok(response.json().await?)
    }

    // Submits a post request to the JS Toolkit Executor
    async fn submit_post_request(
        &self,
        endpoint: &str,
        input_data_json: &JsonValue,
        header_values: &JsonValue,
    ) -> Result<JsonValue, ToolError> {
        let client = Client::new();
        let address = match self {
            JSToolkitExecutor::Local(process) => &process.address,
            JSToolkitExecutor::Remote(remote) => &remote.address,
        };

        let mut request_builder = client
            .post(format!("{}{}", address, endpoint))
            .header("Content-Type", "application/json")
            .json(input_data_json);

        if let JsonValue::Object(headers) = header_values {
            for (key, value) in headers {
                if let Some(value_str) = value.as_str() {
                    request_builder = request_builder.header(key, value_str);
                } else {
                    request_builder = request_builder.header(key, value.to_string());
                }
            }
        }

        let response = request_builder.send().await?;

        Ok(response.json().await?)
    }
}

pub struct RemoteJSToolkitExecutor {
    address: String, // Expected http://ip:port or DNS address
}

pub struct JSToolkitExecutorProcess {
    child: Child,
    address: String,
}

impl JSToolkitExecutorProcess {
    /// Starts the JSToolkitExecutor process, which gets killed if the
    /// the `JSToolkitExecutorProcess` struct gets dropped.
    pub fn start(executor_file_path: &str) -> io::Result<JSToolkitExecutor> {
        let dev_null = if cfg!(windows) {
            File::open("NUL").unwrap()
        } else {
            File::open("/dev/null").unwrap()
        };

        let child = Command::new("node")
            .arg(executor_file_path)
            .arg("-w")
            .arg("-p")
            .arg(*DEFAULT_LOCAL_TOOLKIT_EXECUTOR_PORT)
            .stdout(Stdio::from(dev_null.try_clone().unwrap())) // Redirect stdout
            .stderr(Stdio::from(dev_null)) // Redirect stderr
            .spawn()?;

        let address = format!("http://0.0.0.0:{}", *DEFAULT_LOCAL_TOOLKIT_EXECUTOR_PORT);

        // Wait for 1/2 of a second for the JSToolkitExecutor process to boot up/initialize its
        // web server
        let duration = Duration::from_millis(500);
        thread::sleep(duration);
        Ok(JSToolkitExecutor::Local(JSToolkitExecutorProcess {
            child,
            address,
        }))
    }
}

impl Drop for JSToolkitExecutorProcess {
    fn drop(&mut self) {
        match self.child.kill() {
            Ok(_) => println!("Successfully killed the js-toolkit-executor server process."),
            Err(e) => println!("Failed to kill the js-toolkit-executor server process: {}", e),
        }
    }
}
