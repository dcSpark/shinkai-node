use std::any::Any;
use std::sync::Arc;
use std::collections::HashMap;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::dsl_chain::dsl_inference_chain::DslChain;
use crate::llm_provider::execution::chains::dsl_chain::generic_functions::RustToolFunctions;
use crate::llm_provider::execution::chains::inference_chain_trait::{InferenceChain, InferenceChainContextTrait};
use crate::llm_provider::providers::shared::openai::{FunctionCall, FunctionCallResponse};
use crate::tools::rust_tools::RustTool;
use crate::tools::shinkai_tool::ShinkaiTool;
use crate::tools::tool_router::ToolRouter;
use crate::workflows::sm_executor::AsyncFunction;

pub async fn call_function(
    tool_router: &ToolRouter,
    function_call: FunctionCall,
    db: Arc<ShinkaiDB>,
    context: &dyn InferenceChainContextTrait,
    shinkai_tool: &ShinkaiTool,
    user_profile: &ShinkaiName,
) -> Result<FunctionCallResponse, LLMProviderError> {
    let function_name = function_call.name.clone();
    let function_args = function_call.arguments.clone();

    match shinkai_tool {
        ShinkaiTool::Rust(_) => {
            if let Some(rust_function) = RustToolFunctions::get_tool_function(&function_name) {
                let args: Vec<Box<dyn Any + Send>> = RustTool::convert_args_from_fn_call(function_args)?;
                let result = rust_function(context, args)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                let result_str = result
                    .downcast_ref::<String>()
                    .ok_or_else(|| {
                        LLMProviderError::InvalidFunctionResult(format!("Invalid result: {:?}", result))
                    })?
                    .clone();
                return Ok(FunctionCallResponse {
                    response: result_str,
                    function_call,
                });
            }
        }
        ShinkaiTool::JS(js_tool) => {
            let result = js_tool
                .run(function_args)
                .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
            let result_str = serde_json::to_string(&result)
                .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
            return Ok(FunctionCallResponse {
                response: result_str,
                function_call,
            });
        }
        ShinkaiTool::JSLite(js_lite_tool) => {
            let tool_key =
                ShinkaiTool::gen_router_key(js_lite_tool.name.clone(), js_lite_tool.toolkit_name.clone());
            let full_js_tool = db.get_shinkai_tool(&tool_key, user_profile).map_err(|e| {
                LLMProviderError::FunctionExecutionError(format!("Failed to fetch tool from DB: {}", e))
            })?;

            if let ShinkaiTool::JS(js_tool) = full_js_tool {
                let result = js_tool
                    .run(function_args)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                let result_str = serde_json::to_string(&result)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                return Ok(FunctionCallResponse {
                    response: result_str,
                    function_call,
                });
            } else {
                return Err(LLMProviderError::FunctionNotFound(function_name));
            }
        }
        ShinkaiTool::Workflow(workflow_tool) => {
            let functions: HashMap<String, Box<dyn AsyncFunction>> = HashMap::new();

            let mut dsl_inference =
                DslChain::new(Box::new(context.clone_box()), workflow_tool.workflow.clone(), functions);

            dsl_inference.add_inference_function();
            dsl_inference.add_inference_no_ws_function();
            dsl_inference.add_opinionated_inference_function();
            dsl_inference.add_opinionated_inference_no_ws_function();
            dsl_inference.add_multi_inference_function();
            dsl_inference.add_all_generic_functions();
            dsl_inference.add_tools_from_router().await?;

            let inference_result = dsl_inference.run_chain().await?;

            return Ok(FunctionCallResponse {
                response: inference_result.response,
                function_call,
            });
        }
    }

    Err(LLMProviderError::FunctionNotFound(function_name))
}