use std::{collections::HashMap, env, path::PathBuf};

use serde_json::Value;
use shinkai_tools_runner::tools::{
    code_files::CodeFiles, deno_runner::DenoRunner, deno_runner_options::DenoRunnerOptions, execution_context::ExecutionContext, python_runner::PythonRunner, python_runner_options::PythonRunnerOptions, runner_type::RunnerType
};
pub mod functions;
mod test_utils;

fn get_deno_binary_path() -> PathBuf {
    PathBuf::from(
        env::var("SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH")
            .unwrap_or_else(|_| "./shinkai-tools-runner-resources/deno".to_string()),
    )
}

fn get_uv_binary_path() -> PathBuf {
    PathBuf::from(
        env::var("SHINKAI_TOOLS_RUNNER_UV_BINARY_PATH")
            .unwrap_or_else(|_| "./shinkai-tools-runner-resources/uv".to_string()),
    )
}

fn get_runner_storage_path() -> PathBuf {
    PathBuf::from(env::var("NODE_STORAGE_PATH").unwrap_or_else(|_| "./".to_string())).join("internal_tools_storage")
}

fn get_deno_runner(function_name: &str, code: String, configurations: Value, mount_files: Vec<PathBuf>) -> DenoRunner {
    DenoRunner::new(
        CodeFiles {
            files: HashMap::from([("main.ts".to_string(), code)]),
            entrypoint: "main.ts".to_string(),
        },
        configurations,
        Some(DenoRunnerOptions {
            deno_binary_path: get_deno_binary_path(),
            context: ExecutionContext {
                storage: get_runner_storage_path(),
                context_id: function_name.to_string(),
                mount_files,
                ..Default::default()
            },
            force_runner_type: Some(RunnerType::Host),
            ..Default::default()
        }),
    )
}

#[derive(Debug, Clone, Copy)]
pub enum NonRustRuntime {
    Deno,
    Python,
}

fn get_python_binary_path() -> PathBuf {
    PathBuf::from(env::var("SHINKAI_TOOLS_RUNNER_PYTHON_BINARY_PATH").unwrap_or_else(|_| "python3".to_string()))
}

fn get_python_runner(
    function_name: &str,
    code: String,
    configurations: Value,
    mount_files: Vec<PathBuf>,
) -> PythonRunner {
    PythonRunner::new(
        CodeFiles {
            files: HashMap::from([("main.py".to_string(), code)]),
            entrypoint: "main.py".to_string(),
        },
        configurations,
        Some(PythonRunnerOptions {
            uv_binary_path: get_uv_binary_path(),
            context: ExecutionContext {
                storage: get_runner_storage_path(),
                context_id: function_name.to_string(),
                mount_files,
                ..Default::default()
            },
            force_runner_type: Some(RunnerType::Host),
            ..Default::default()
        }),
    )
}

#[derive(Debug)]
pub enum RunError {
    CodeExecutionError(String),
    SerializeConfigurationsError(String),
    SerializeParamsError(String),
    ParseOutputError(String),
}

pub struct NonRustCodeRunnerFactory {
    function_name: String,
    code: String,
    mount_files: Vec<PathBuf>,
    runtime: NonRustRuntime,
}

impl NonRustCodeRunnerFactory {
    pub fn new(function_name: impl Into<String>, code: impl Into<String>, mount_files: Vec<PathBuf>) -> Self {
        Self {
            function_name: function_name.into(),
            code: code.into(),
            mount_files,
            runtime: NonRustRuntime::Deno,
        }
    }

    pub fn with_runtime(mut self, runtime: NonRustRuntime) -> Self {
        self.runtime = runtime;
        self
    }

    pub fn create_runner<C>(&self, configurations: C) -> NonRustCodeRunner<C>
    where
        C: serde::Serialize,
    {
        NonRustCodeRunner {
            function_name: self.function_name.clone(),
            code: self.code.clone(),
            configurations,
            mount_files: self.mount_files.clone(),
            runtime: self.runtime,
        }
    }
}

pub struct NonRustCodeRunner<C> {
    function_name: String,
    code: String,
    configurations: C,
    mount_files: Vec<PathBuf>,
    runtime: NonRustRuntime,
}

impl<C> NonRustCodeRunner<C>
where
    C: serde::Serialize,
{
    pub async fn run<P, T>(&self, params: P) -> Result<T, RunError>
    where
        P: serde::Serialize,
        T: serde::de::DeserializeOwned,
    {
        let configurations_value = serde_json::to_value(&self.configurations)
            .map_err(|e| RunError::SerializeConfigurationsError(e.to_string()))?;
        let params_value = serde_json::to_value(params).map_err(|e| RunError::SerializeParamsError(e.to_string()))?;
        let result = match self.runtime {
            NonRustRuntime::Deno => {
                let deno_runner = get_deno_runner(
                    &self.function_name,
                    self.code.clone(),
                    configurations_value,
                    self.mount_files.clone(),
                );
                deno_runner
                    .run(None, params_value, None)
                    .await
                    .map_err(|e| RunError::CodeExecutionError(e.to_string()))?
            }
            NonRustRuntime::Python => {
                let python_runner = get_python_runner(
                    &self.function_name,
                    self.code.clone(),
                    configurations_value,
                    self.mount_files.clone(),
                );
                python_runner
                    .run(None, params_value, None)
                    .await
                    .map_err(|e| RunError::CodeExecutionError(e.to_string()))?
            }
        };
        serde_json::from_value(result.data).map_err(|e| RunError::ParseOutputError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Deserialize)]
    struct TestOutput {
        message: String,
    }

    #[tokio::test]
    async fn test_non_rust_code_runner() {
        let code = r#"
            async function run(configurations, params) {
                return {
                    message: `Hello ${params.name}!`
                };
            }
        "#
        .to_string();

        let runner = NonRustCodeRunnerFactory::new("test_function", code, vec![]).create_runner(json!({}));

        let result = runner
            .run::<_, TestOutput>(json!({
                "name": "World"
            }))
            .await
            .unwrap();

        assert_eq!(result.message, "Hello World!");
    }
}
