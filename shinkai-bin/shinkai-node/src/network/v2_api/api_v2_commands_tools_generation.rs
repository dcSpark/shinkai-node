use crate::{
    llm_provider::job_manager::JobManager,
    managers::{tool_router::ToolRouter, IdentityManager},
    network::{node_error::NodeError, Node},
    tools::{
        tool_definitions::definition_generation::{generate_tool_definitions, get_all_deno_tools},
        tool_generation::v2_create_and_send_job_message,
        tool_prompts::{generate_code_prompt, tool_metadata_implementation_prompt},
    },
};
use async_channel::Sender;
use ed25519_dalek::SigningKey;
use reqwest::StatusCode;
use serde_json::{json, Value};
use shinkai_http_api::node_api_router::{APIError, SendResponseBodyData};
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName, job::JobLike, shinkai_name::ShinkaiName,
        shinkai_tools::{CodeLanguage, DynamicToolType},
        tool_router_key::ToolRouterKey,
    },
    shinkai_message::shinkai_message_schemas::{CallbackAction, JobMessage},
    shinkai_utils::job_scope::MinimalJobScope,
};
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

impl Node {
    pub async fn generate_tool_fetch_query(
        bearer: String,
        db: Arc<SqliteManager>,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        code: String,
        identity_manager: Arc<Mutex<IdentityManager>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let tool_definitions = match generate_tool_definitions(tools.clone(), language.clone(), db.clone(), true).await
        {
            Ok(definitions) => definitions,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate tool definitions: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let is_memory_required = tools.iter().any(|tool| {
            tool.to_string_without_version() == "local:::__official_shinkai:::shinkai_sqlite_query_executor"
        });
        let code_prompt =
            match generate_code_prompt(language.clone(), is_memory_required, "".to_string(), tool_definitions).await {
                Ok(prompt) => prompt,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to generate code prompt: {:?}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };

        let metadata_prompt = match tool_metadata_implementation_prompt(
            language.clone(),
            code.clone(),
            tools.clone(),
            identity_manager.clone(),
        )
        .await
        {
            Ok(prompt) => prompt,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate tool definitions: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let all_tools: Vec<ToolRouterKey> = db
            .clone()
            .get_all_tool_headers()?
            .into_iter()
            .filter_map(|tool| match ToolRouterKey::from_string(&tool.tool_router_key) {
                Ok(tool_router_key) => Some(tool_router_key),
                Err(_) => None,
            })
            .collect();
        let library_code = match generate_tool_definitions(all_tools, language.clone(), db.clone(), false).await {
            Ok(code) => code,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate tool definitions: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let header_code = match generate_tool_definitions(tools.clone(), language.clone(), db.clone(), true).await {
            Ok(code) => code,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate tool definitions: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let _ = res
            .send(Ok(json!({
                "availableTools": get_all_deno_tools(db.clone()).await.into_iter().map(|tool| tool.tool_router_key).collect::<Vec<String>>(),
                "libraryCode": library_code.clone(),
                "headers": header_code.clone(),
                "codePrompt": code_prompt.clone(),
                "metadataPrompt": metadata_prompt.clone(),
            })))
            .await;
        Ok(())
    }

    async fn is_code_generator(
        db: Arc<SqliteManager>,
        job_id: &str,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
    ) -> bool {
        let llm_provider = match db.get_job_with_options(job_id, false) {
            Ok(job) => job.parent_agent_or_llm_provider_id.clone(),
            Err(_) => return false,
        };

        let main_identity = {
            let identity_manager = identity_manager_clone.lock().await;
            match identity_manager.get_main_identity() {
                Some(identity) => identity.clone(),
                None => return false,
            }
        };

        let sender = match ShinkaiName::new(main_identity.get_full_identity_name()) {
            Ok(name) => name,
            Err(_) => return false,
        };

        match db.get_llm_provider(&llm_provider, &sender) {
            Ok(llm_provider) => {
                if let Some(llm_provider) = llm_provider {
                    let provider = llm_provider.get_provider_string();
                    let model = llm_provider.get_model_string().to_lowercase();
                    println!("provider: {}", provider);
                    println!("model: {}", model);

                    if provider == "shinkai-backend"
                        && (model == "code_generator" || model == "code_generator_no_feedback")
                    {
                        return true;
                    }
                }
            }
            Err(_) => return false,
        };
        return false;
    }

    pub async fn generate_tool_implementation(
        bearer: String,
        db: Arc<SqliteManager>,
        job_message: JobMessage,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        post_check: bool,
        raw: bool,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let is_code_generator =
            Self::is_code_generator(db.clone(), &job_message.job_id, identity_manager_clone.clone()).await;

        println!("is_code_generator: {}", is_code_generator);

        let tools = if is_code_generator {
            let valid_tool_list: Vec<String> = vec![
                "local:::__official_shinkai:::shinkai_llm_prompt_processor",
                "local:::__official_shinkai:::x_twitter_post",
                "local:::__official_shinkai:::duckduckgo_search",
                "local:::__official_shinkai:::x_twitter_search",
            ]
            .iter()
            .map(|t| t.to_string())
            .collect();
            let user_tools: Vec<String> = tools.iter().map(|tools| tools.to_string_with_version()).collect();
            let all_tool_headers = db.clone().get_all_tool_headers()?;
            all_tool_headers
                .into_iter()
                .map(|tool| ToolRouterKey::from_string(&tool.tool_router_key))
                .filter(|tool| tool.is_ok())
                .map(|tool| tool.unwrap())
                .filter(|tool| {
                    let t = tool.to_string_without_version();
                    user_tools.contains(&t) || valid_tool_list.contains(&t)
                })
                .collect::<Vec<ToolRouterKey>>()
        } else {
            tools.clone()
        };

        let tool_definitions = match generate_tool_definitions(tools.clone(), language.clone(), db.clone(), true).await
        {
            Ok(definitions) => definitions,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate tool definitions: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let prompt = job_message.content.clone();
        let is_memory_required = tools.clone().iter().any(|tool| {
            tool.to_string_without_version() == "local:::__official_shinkai:::shinkai_sqlite_query_executor"
        });

        let generate_code_prompt = match raw {
            true => prompt,
            false => match generate_code_prompt(language.clone(), is_memory_required, prompt, tool_definitions).await {
                Ok(prompt) => prompt,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to generate code prompt: {:?}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            },
        };

        if let Err(err) = Self::disable_tools_for_job(db.clone(), bearer.clone(), job_message.job_id.clone()).await {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: err,
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let mut job_message_clone = job_message.clone();
        job_message_clone.content = generate_code_prompt;

        if post_check {
            let callback_action =
                CallbackAction::ImplementationCheck(language.to_dynamic_tool_type().unwrap(), tools.clone());
            job_message_clone.callback = Some(Box::new(callback_action));
        }

        Node::v2_job_message(
            db,
            node_name_clone,
            identity_manager_clone,
            job_manager_clone,
            bearer,
            job_message_clone,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
            Some(true),
            res,
        )
        .await
    }

    pub async fn generate_tool_metadata_implementation(
        bearer: String,
        job_id: String,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        db: Arc<SqliteManager>,
        node_name_clone: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let job = match db.get_job_with_options(&job_id, true) {
            Ok(job) => job,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve job: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        if let Err(err) = Self::disable_tools_for_job(db.clone(), bearer.clone(), job_id.clone()).await {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: err,
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let last_message = {
            let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string())?;
            let messages = match db.get_last_messages_from_inbox(inbox_name.to_string(), 2, None) {
                Ok(messages) => messages,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to retrieve last messages from inbox: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };
            if messages.len() < 2 {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "Most likely the LLM hasn't processed the code task yet".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            };

            if let Some(last_message) = messages.last().and_then(|msg| msg.last()) {
                last_message.clone()
            } else {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "Failed to retrieve the last message".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let code = match last_message.get_message_content() {
            Ok(code) => code,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve the last message content: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let language_str = match language.clone() {
            CodeLanguage::Typescript => "typescript",
            CodeLanguage::Python => "python",
        };
        let start_pattern = &format!("```{}", language_str);
        let end_pattern = "```";
        let code = if code.contains(start_pattern) {
            let start = code.find(start_pattern).unwrap_or(0);
            let end = code[(start + start_pattern.len())..]
                .find(end_pattern)
                .map(|i| i + start + start_pattern.len())
                .unwrap_or(code.len());

            let content_start = if code[start..].starts_with(start_pattern) {
                start + start_pattern.len()
            } else {
                start
            };

            code[content_start..end].trim().to_string()
        } else {
            code
        };

        let metadata_prompt = match tool_metadata_implementation_prompt(
            language.clone(),
            code.clone(),
            tools.clone(),
            identity_manager.clone(),
        )
        .await
        {
            Ok(prompt) => prompt,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate tool definitions: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let job_message = v2_create_and_send_job_message(
            job.clone(),
            metadata_prompt,
            db.clone(),
            node_name_clone,
            identity_manager,
            job_manager_clone,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
            Some(CallbackAction::MetadataImplementationCheck(
                language.to_dynamic_tool_type().unwrap(),
                tools.clone(),
                code.clone(),
            )),
        )
        .await?;

        let _ = res.send(Ok(json!({ "job_id": job_message.job_id }))).await;
        Ok(())
    }

    pub async fn disable_tools_for_job(
        db: Arc<SqliteManager>,
        bearer: String,
        job_id: String,
    ) -> Result<(), String> {
        let job = match db.get_job_with_options(&job_id, false) {
            Ok(job) => job,
            Err(err) => return Err(format!("Failed to retrieve job: {}", err)),
        };

        let mut job_clone = job.clone();
        job_clone.disable_tools = true;

        match db.update_job(&job_clone) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Failed to update job: {}", err)),
        }
    }
}
