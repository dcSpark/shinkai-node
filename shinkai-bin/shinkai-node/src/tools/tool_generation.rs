use ed25519_dalek::SigningKey;
use reqwest::StatusCode;
use serde_json::{json, Value};
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::api_v2::api_v2_handlers_tools::Language;
use shinkai_http_api::node_api_router::APIError;
use shinkai_lancedb::lance_db::shinkai_lance_db::LanceShinkaiDb;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::managers::IdentityManager;
use crate::{llm_provider::job_manager::JobManager, network::Node};

use std::fmt;

use super::generate_tool_definitions;

// Define the ToolImplementationError type
#[derive(Debug)]
pub struct ToolImplementationError {
    pub message: String,
    pub code: Option<u16>, // Optional HTTP status code
}

// Implement the Display trait for ToolImplementationError
impl fmt::Display for ToolImplementationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ToolImplementationError: {}", self.message)
    }
}

// Implement the Error trait for ToolImplementationError
impl std::error::Error for ToolImplementationError {}

// Implement a constructor for ToolImplementationError
impl ToolImplementationError {
    pub fn new(message: String, code: Option<u16>) -> Self {
        ToolImplementationError { message, code }
    }

    // You can add more methods as needed
}

pub async fn tool_implementation(
    bearer: String,
    language: Language,
    code: Option<String>,
    metadata: Option<String>,
    output: Option<String>,
    prompt: Option<String>,
    lance_db: Arc<RwLock<LanceShinkaiDb>>,
    db_clone: Arc<ShinkaiDB>,
    node_name_clone: ShinkaiName,
    identity_manager_clone: Arc<Mutex<IdentityManager>>,
    job_manager_clone: Arc<Mutex<JobManager>>,
    job_creation_info: JobCreationInfo,
    llm_provider: String,
    encryption_secret_key_clone: EncryptionStaticKey,
    encryption_public_key_clone: EncryptionPublicKey,
    signing_secret_key_clone: SigningKey,
    raw: bool,
    fetch_query: bool,
) -> Result<Value, APIError> {
    // Generate tool definitions first
    let tool_definitions = generate_tool_definitions(language.clone(), lance_db.clone()).await;
    let mut generate_code_prompt = String::new();

    if (raw == false) {
        if let Some(prompt_text) = prompt {
            match language {
                Language::Typescript => {
                    generate_code_prompt.push_str(&format!(
                        "
RULE I: 
These are two examples of METADATA:
{{
  id: 'shinkai-tool-coinbase-create-wallet',
  name: 'Shinkai: Coinbase Wallet Creator',
  description: 'Tool for creating a Coinbase wallet',
  author: 'Shinkai',
  keywords: ['coinbase', 'wallet', 'creator', 'shinkai'],
  configurations: {{
    type: 'object',
    properties: {{
      name: {{ type: 'string' }},
      privateKey: {{ type: 'string' }},
      useServerSigner: {{ type: 'string', default: 'false', nullable: true }},
    }},
    required: [{{'name', 'privateKey'}}],
  }},
  parameters: {{
    type: 'object',
    properties: {{}},
    required: [], // No required parameters
  }},
  result: {{
    type: 'object',
    properties: {{
      walletId: {{ type: 'string', nullable: true }},
      seed: {{ type: 'string', nullable: true }},
      address: {{ type: 'string', nullable: true }},
    }},
    required: [],
  }},
}};

{{
  id: 'shinkai-tool-download-pages',
  name: 'Shinkai: Download Pages',
  description: 'Downloads one or more URLs and converts their HTML content to Markdown',
  author: 'Shinkai',
  keywords: [
    'HTML to Markdown',
    'web page downloader',
    'content conversion',
    'URL to Markdown',
  ],
  configurations: {{
    type: 'object',
    properties: {{}},
    required: [],
  }},
  parameters: {{
    type: 'object',
    properties: {{
      urls: {{ type: 'array', items: {{ type: 'string' }} }},
    }},
    required: [{{'urls'}}],
  }},
  result: {{
    type: 'object',
    properties: {{
      markdowns: {{ type: 'array', items: {{ type: 'string' }} }},
    }},
    required: [{{'markdowns'}}],
  }},
}};

---- 
RULE II:
Following this example, generate the METADATA for the following code in the {} language:

{}

",
                        language,
                        code.unwrap_or("".to_string())
                    ));
                }
                Language::Python => {
                    return Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("NYI Python"),
                    })
                }
                _ => {
                    return Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Unknown Language {}", language),
                    });
                }
            }
        }
    } else {
        generate_code_prompt = prompt.unwrap_or("".to_string());
    }

    if (fetch_query) {
        return Ok(json!({
            "query": generate_code_prompt,
        }));
    }

    let (res_sender, res_receiver) = async_channel::bounded(1);

    let _ = Node::v2_create_new_job(
        db_clone.clone(),
        node_name_clone.clone(),
        identity_manager_clone.clone(),
        job_manager_clone.clone(),
        bearer.clone(),
        job_creation_info,
        llm_provider,
        encryption_secret_key_clone.clone(),
        encryption_public_key_clone.clone(),
        signing_secret_key_clone.clone(),
        res_sender,
    )
    .await;

    let result = res_receiver.recv().await.map_err(|e| {
        // Convert the Rejection error to APIError
        APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Error receiving result: {:?}", e),
        }
    })?;
    let job_id = match result {
        Ok(job_id) => job_id,
        Err(e) => {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Error receiving result: {:?}", e),
            };
            return Err(api_error);
        }
    };

    let job_message = JobMessage {
        job_id: job_id.clone(),
        content: generate_code_prompt,
        files_inbox: "".to_string(),
        parent: None,
        workflow_code: None,
        workflow_name: None,
        sheet_job_data: None,
        callback: None,
        metadata: None,
    };

    let (res_sender, res_receiver) = async_channel::bounded(1);

    let _ = Node::v2_job_message(
        db_clone.clone(),
        node_name_clone.clone(),
        identity_manager_clone.clone(),
        job_manager_clone.clone(),
        bearer.clone(),
        job_message,
        encryption_secret_key_clone.clone(),
        encryption_public_key_clone.clone(),
        signing_secret_key_clone.clone(),
        res_sender,
    )
    .await;

    let result = res_receiver.recv().await.map_err(|e| {
        // Convert the Rejection error to APIError
        APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Error receiving result: {:?}", e),
        }
    })?;

    match result {
        Ok(_) => Ok(json!({
            "job_id": job_id,
        })),
        Err(e) => {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Error receiving result: {:?}", e),
            };
            return Err(api_error);
        }
    }
}

pub async fn tool_metadata_implementation(
    bearer: String,
    language: Language,
    code: Option<String>,
    metadata: Option<String>,
    output: Option<String>,
    lance_db: Arc<RwLock<LanceShinkaiDb>>,
    db_clone: Arc<ShinkaiDB>,
    node_name_clone: ShinkaiName,
    identity_manager_clone: Arc<Mutex<IdentityManager>>,
    job_manager_clone: Arc<Mutex<JobManager>>,
    job_creation_info: JobCreationInfo,
    llm_provider: String,
    encryption_secret_key_clone: EncryptionStaticKey,
    encryption_public_key_clone: EncryptionPublicKey,
    signing_secret_key_clone: SigningKey,
) -> Result<Value, APIError> {
    // Generate tool definitions first
    let tool_definitions = generate_tool_definitions(language.clone(), lance_db.clone()).await;
    let mut generate_code_prompt = String::new();

    match language {
        Language::Typescript => {
            generate_code_prompt.push_str(&format!(
                "
RULE I:
You may use any of the following tools if they are relevant and a good match for the task:

{},
================================================================
RULE II:
Write a metadata definition in {} for the following task, including name, description, and parameters:

type METADATA = {{
name: string;
description: string;
parameters: Record<string, any>;
}};

================================================================
RULE III:
Implement metadata for the following task:

",
                &tool_definitions, language
            ));
        }
        Language::Python => {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("NYI Python"),
            })
        }
        _ => {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Unknown Language {}", language),
            });
        }
    }

    let (res_sender, res_receiver) = async_channel::bounded(1);

    let _ = Node::v2_create_new_job(
        db_clone.clone(),
        node_name_clone.clone(),
        identity_manager_clone.clone(),
        job_manager_clone.clone(),
        bearer.clone(),
        job_creation_info,
        llm_provider,
        encryption_secret_key_clone.clone(),
        encryption_public_key_clone.clone(),
        signing_secret_key_clone.clone(),
        res_sender,
    )
    .await;

    let result = res_receiver.recv().await.map_err(|e| {
        // Convert the Rejection error to APIError
        APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Error receiving result: {:?}", e),
        }
    })?;

    let job_id = match result {
        Ok(job_id) => job_id,
        Err(e) => {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Error receiving result: {:?}", e),
            };
            return Err(api_error);
        }
    };

    let job_message = JobMessage {
        job_id: job_id.clone(),
        content: generate_code_prompt,
        files_inbox: "".to_string(),
        parent: None,
        workflow_code: None,
        workflow_name: None,
        sheet_job_data: None,
        callback: None,
        metadata: None,
    };

    let (res_sender, res_receiver) = async_channel::bounded(1);

    let _ = Node::v2_job_message(
        db_clone.clone(),
        node_name_clone.clone(),
        identity_manager_clone.clone(),
        job_manager_clone.clone(),
        bearer.clone(),
        job_message,
        encryption_secret_key_clone.clone(),
        encryption_public_key_clone.clone(),
        signing_secret_key_clone.clone(),
        res_sender,
    )
    .await;

    let result = res_receiver.recv().await.map_err(|e| {
        // Convert the Rejection error to APIError
        APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Error receiving result: {:?}", e),
        }
    })?;

    match result {
        Ok(_) => Ok(json!({
            "job_id": job_id,
        })),
        Err(e) => {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Error receiving result: {:?}", e),
            };
            return Err(api_error);
        }
    }
}
