use crate::tools::error::ToolError;
use lazy_static::lazy_static;
use reqwest::blocking::Client;
use serde_json::Value as JsonValue;
use std::fs::File;
use std::io;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

lazy_static! {
    pub static ref DEFAULT_LOCAL_TOOLKIT_EXECUTOR_PORT: &'static str = "3555";
}

pub enum JSToolkitExecutor {
    Local(JSToolkitExecutorProcess),
    Remote(RemoteJSToolkitExecutor),
}

impl JSToolkitExecutor {
    // Starts the JS Toolkit Executor locally at default path `./files/shinkai-toolkit-executor.js`
    // Primarily intended for local testing (executor should be sandboxed for production)
    pub fn new_local() -> Result<Self, ToolError> {
        let executor = JSToolkitExecutor::new_local_custom_path("./files/shinkai-toolkit-executor.js")?;
        Ok(executor)
    }

    // Starts the JS Toolkit Executor locally at a custom path
    // Primarily intended for local testing (executor should be sandboxed for production)
    pub fn new_local_custom_path(executor_file_path: &str) -> Result<Self, ToolError> {
        let executor = JSToolkitExecutorProcess::start(executor_file_path)
            .map_err(|_| ToolError::JSToolkitExecutorFailedStarting)?;
        executor.submit_health_check()?;
        Ok(executor)
    }

    // Establishes connection to a remotely ran JS Toolkit Executor
    pub fn new_remote(address: String) -> Result<Self, ToolError> {
        let executor = JSToolkitExecutor::Remote(RemoteJSToolkitExecutor { address });
        executor.submit_health_check()?;
        Ok(executor)
    }

    // Submits a health check request to /health_check and checks the response
    pub fn submit_health_check(&self) -> Result<(), ToolError> {
        let health_check_json = serde_json::json!({});
        let response = self.submit_request("/healthcheck", &health_check_json)?;
        if response.get("status").unwrap_or(&JsonValue::Bool(false)) == &JsonValue::Bool(true) {
            Ok(())
        } else {
            Err(ToolError::JSToolkitExecutorNotAvailable)
        }
    }

    pub fn submit_tool_execution_request(&self, input_data_json: &JsonValue) -> Result<JsonValue, ToolError> {
        self.submit_request("/exec", input_data_json)
    }

    fn submit_request(&self, endpoint: &str, input_data_json: &JsonValue) -> Result<JsonValue, ToolError> {
        let client = Client::new();
        let address = match self {
            JSToolkitExecutor::Local(process) => &process.address,
            JSToolkitExecutor::Remote(remote) => &remote.address,
        };

        let response = client
            .post(&format!("{}{}", address, endpoint))
            .header("Content-Type", "application/json")
            .json(input_data_json)
            .send()
            .map_err(|_| ToolError::FailedJSONParsing)?;

        response.json().map_err(|_| ToolError::FailedJSONParsing)
    }
}

pub struct RemoteJSToolkitExecutor {
    address: String, // Expected ip:port or DNS address
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
            .arg(format!("{}", DEFAULT_LOCAL_TOOLKIT_EXECUTOR_PORT.to_string()))
            .stdout(Stdio::from(dev_null.try_clone().unwrap())) // Redirect stdout
            .stderr(Stdio::from(dev_null)) // Redirect stderr
            .spawn()?;

        let address = format!("0.0.0.0:{}", DEFAULT_LOCAL_TOOLKIT_EXECUTOR_PORT.to_string());

        // Wait for 1/10th of a second for the JSToolkitExecutor process to boot up/initialize its
        // web server
        let duration = Duration::from_millis(100);
        thread::sleep(duration);
        Ok(JSToolkitExecutor::Local(JSToolkitExecutorProcess {
            child,
            address: address,
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
