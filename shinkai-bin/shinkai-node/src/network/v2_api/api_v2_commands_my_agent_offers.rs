use crate::{
    llm_provider::job_manager::JobManager,
    managers::IdentityManager,
    network::{
        agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager, node_error::NodeError, Node,
    },
    tools::tool_implementation::{
        native_tools::llm_prompt_processor::LlmPromptProcessorTool, tool_traits::ToolExecutor,
    },
};
use async_channel::Sender;
use ed25519_dalek::SigningKey;
use reqwest::StatusCode;
use serde_json::{Map, Value};
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tool_offering::UsageTypeInquiry;
use shinkai_sqlite::{errors::SqliteManagerError, SqliteManager};
use shinkai_tools_primitives::tools::{error::ToolError, shinkai_tool::ShinkaiTool};
use std::sync::Arc;
use tokio::sync::Mutex;
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

impl Node {
    pub async fn v2_api_request_invoice(
        db: Arc<SqliteManager>,
        my_agent_payments_manager: Arc<Mutex<MyAgentOfferingsManager>>,
        bearer: String,
        tool_key_name: String,
        usage: UsageTypeInquiry,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Fetch the tool from lance_db
        let network_tool = {
            match db.get_tool_by_key(&tool_key_name) {
                Ok(tool) => match tool {
                    ShinkaiTool::Network(network_tool, _) => network_tool,
                    _ => {
                        let api_error = APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: "Tool is not a NetworkTool".to_string(),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                },
                Err(SqliteManagerError::ToolNotFound(_)) => {
                    let api_error = APIError {
                        code: StatusCode::NOT_FOUND.as_u16(),
                        error: "Not Found".to_string(),
                        message: "Tool not found in LanceShinkaiDb".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to fetch tool from LanceShinkaiDb: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        };

        // Lock the payments manager
        let manager = my_agent_payments_manager.lock().await;

        // Request the invoice
        match manager.network_request_invoice(network_tool, usage).await {
            Ok(invoice_request) => {
                let invoice_value = match serde_json::to_value(invoice_request) {
                    Ok(value) => value,
                    Err(e) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to serialize invoice request: {}", e),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                };
                let _ = res.send(Ok(invoice_value)).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to request invoice: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_pay_invoice(
        db: Arc<SqliteManager>,
        my_agent_offerings_manager: Arc<Mutex<MyAgentOfferingsManager>>,
        bearer: String,
        invoice_id: String,
        data_for_tool: Value,
        node_name: ShinkaiName,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Step 1: Get the invoice from the database
        let invoice = match db.get_invoice(&invoice_id) {
            Ok(invoice) => invoice,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get invoice: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Step 2: Verify the invoice
        let is_valid = match my_agent_offerings_manager.lock().await.verify_invoice(&invoice).await {
            Ok(valid) => valid,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to verify invoice: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        if !is_valid {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invoice is not valid".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Step 3: Check that the invoice is not expired
        if invoice.expiration_time < chrono::Utc::now() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invoice has expired".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Step 4: Check that the data_for_tool is valid
        let tool_key_name = invoice.shinkai_offering.tool_key.clone();
        let tool = {
            match db.get_tool_by_key(&tool_key_name) {
                Ok(tool) => tool,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to fetch tool from LanceShinkaiDb: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        };

        // Check if the tool has the required input_args
        let required_args = match tool {
            ShinkaiTool::Network(network_tool, _) => network_tool.input_args,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Tool is not a NetworkTool".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validate that data_for_tool contains all the required input_args
        for arg in required_args
            .to_deprecated_arguments()
            .iter()
            .filter(|arg| arg.is_required)
        {
            if !data_for_tool.get(&arg.name).is_some() {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Missing required argument: {}", arg.name),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }

        // Step 5: Pay the invoice
        let payment = match my_agent_offerings_manager
            .lock()
            .await
            .pay_invoice_and_send_receipt(invoice_id, data_for_tool, node_name.clone())
            .await
        {
            Ok(payment) => payment,
            Err(e) => {
                // Use regex to extract a more human-readable error message
                let error_message = e.to_string();
                let human_readable_message = if let Ok(regex) = regex::Regex::new(r#"message: \\"(.*?)\\""#) {
                    if let Some(captures) = regex.captures(&error_message) {
                        captures
                            .get(1)
                            .map_or(error_message.clone(), |m| m.as_str().to_string())
                    } else {
                        error_message.clone()
                    }
                } else {
                    error_message.clone()
                };

                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to pay invoice: {}", human_readable_message),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Send success response with payment details
        let payment_value = match serde_json::to_value(payment) {
            Ok(value) => value,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to serialize payment: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let _ = res.send(Ok(payment_value)).await;
        Ok(())
    }

    pub async fn v2_api_list_invoices(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Fetch the list of invoices from the database
        match db.get_all_invoices() {
            Ok(invoices) => {
                let invoices_value = match serde_json::to_value(invoices) {
                    Ok(value) => value,
                    Err(e) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to serialize invoices: {}", e),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                };
                let _ = res.send(Ok(invoices_value)).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to fetch invoices: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_generate_agent_from_prompt(
        db: Arc<SqliteManager>,
        bearer: String,
        prompt: String,
        llm_provider: String,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        signing_secret_key: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let mut retries = 3;

        let mut agent;
        // We are using a LLM to generate the agent.
        // So the return type might be flacky.
        // We will retry 3 times.
        loop {
            // Create a new agent from the prompt
            agent = Self::create_agent_from_prompt(
                prompt.clone(),
                llm_provider.clone(),
                db.clone(),
                bearer.clone(),
                node_name.clone(),
                identity_manager.clone(),
                job_manager.clone(),
                encryption_secret_key.clone(),
                encryption_public_key.clone(),
                signing_secret_key.clone(),
            )
            .await;

            if let Err(e) = agent {
                if retries > 0 {
                    retries -= 1;
                    continue;
                }
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate agent: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
            break;
        }

        let _ = res.send(Ok(agent.unwrap())).await;
        Ok(())
    }

    async fn create_agent_from_prompt(
        agent_prompt: String,
        llm_provider: String,
        db: Arc<SqliteManager>,
        bearer: String,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        signing_secret_key: SigningKey,
    ) -> Result<Value, ToolError> {
        let static_prompt = r#"
<intro>
  * You are a generator of AGENT definitions.
  * An AGENT contains a name, indications, instructions and a list of tools to achieve a specific goal.
  * An AGENT is specified in JSON format.
  * An AGENT will later be called by the user with a prompt.
  * The "rules" tag has the instructions you must follow to generate the AGENT.
  * The "command" tag has the definition of the AGENT.
  * The "output" tag has an example of the output you must generate.
  * Do not include any other text than the JSON.
</intro>

<rules>
  * The name must be concise.
  * The indications must be a short description of the AGENT.
  * The instructions, is the system prompt, the mind of the AGENT. It must be a markdown of steps, actions to achieve the goal or goals of the AGENT.
  * The tools must be a list of tools that the AGENT will need to achieve the goal.
  * A tool must description must contain: it's action, the expected inputs and return object.
  * The output must be a valid JSON.
</rules>

<output>
```json
  {
    "name": "AGENT_NAME",
    "indications": "AGENT_INDICATIONS",
    "instructions": "AGENT_INSTRUCTIONS",
    "tools": [{
        "name": "TOOL_NAME",
        "description": "TOOL_DESCRIPTION",
    }]
  }
```
</output>
"#;

        let prompt = format!(
            "{}

<command>
{}
</command>
",
            static_prompt, agent_prompt
        );
        let mut parameters = Map::new();
        parameters.insert("prompt".to_string(), prompt.clone().into());
        parameters.insert("llm_provider".to_string(), llm_provider.clone().into());

        let body = LlmPromptProcessorTool::execute(
            bearer,
            "tool_id".to_string(),
            "app_id".to_string(),
            db,
            node_name,
            identity_manager,
            job_manager,
            encryption_secret_key,
            encryption_public_key,
            signing_secret_key,
            &parameters,
            llm_provider,
        )
        .await?;

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

        serde_json::from_str::<serde_json::Value>(&cleaned_json)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to parse JSON: {}", e)))
    }
}
