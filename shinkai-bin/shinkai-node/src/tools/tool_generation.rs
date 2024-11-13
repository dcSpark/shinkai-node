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

fn generic_error(e: impl std::error::Error) -> APIError {
    APIError {
        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        error: "Internal Server Error".to_string(),
        message: format!("Error receiving result: {:?}", e),
    }
}
fn generic_error_str(e: &str) -> APIError {
    APIError {
        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        error: "Internal Server Error".to_string(),
        message: format!("Error receiving result: {}", e),
    }
}

pub async fn v2_create_and_send_job_message(
    bearer: String,
    job_creation_info: JobCreationInfo,
    llm_provider: String,
    content: String,
    db_clone: Arc<ShinkaiDB>,
    node_name_clone: ShinkaiName,
    identity_manager_clone: Arc<Mutex<IdentityManager>>,
    job_manager_clone: Arc<Mutex<JobManager>>,
    encryption_secret_key_clone: EncryptionStaticKey,
    encryption_public_key_clone: EncryptionPublicKey,
    signing_secret_key_clone: SigningKey,
) -> Result<String, APIError> {
    // Create job
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

    let job_id = res_receiver.recv().await.map_err(|e| generic_error(e))??;

    // Send message
    let job_message = JobMessage {
        job_id: job_id.clone(),
        content,
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
        db_clone,
        node_name_clone,
        identity_manager_clone,
        job_manager_clone,
        bearer,
        job_message,
        encryption_secret_key_clone,
        encryption_public_key_clone,
        signing_secret_key_clone,
        res_sender,
    )
    .await;

    res_receiver.recv().await.map_err(|e| generic_error(e))??;
    Ok(job_id)
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
    let tool_definitions = generate_tool_definitions(language.clone(), lance_db.clone(), true).await?;
    let mut generate_code_prompt = String::new();

    if !raw {
        if let Some(prompt_text) = prompt {
            match language {
                Language::Typescript => {
                    generate_code_prompt.push_str(&format!(
                        "
# RULE I:
* You may use any of the following functions if they are relevant and a good match for the task.
* Import them in the following way (do not rename functions with 'as'):
`import {{ xx }} from '@shinkai/local-tools'`

* This is the content of '@shinkai/local-tools':
```{language}
{tool_definitions}
```

#RULE II:
* To implement the task you can update the CONFIG, INPUTS and OUTPUT types to match the run function type: 
```{language}
type CONFIG = {{}};
type INPUTS = {{}};
type OUTPUT = {{}};
export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {{ 
    return {{}};
}}
```

# RULE III:
* This will be shared as a library, when used it run(...) function will be called.
* The function signature MUST be: `export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT>`
* If you need to import other libraries, do it in the Deno NPM format and with version, for example to import axios use 'import axios from 'npm:axios@1.6.2' with the 'npm:' prefix, and the exact version.

# RULE IV:
* Do not output, notes, ideas, explanations or examples.
* Output only valid {language} code, so the complete Output can be directly executed. 
* Only if required any additional notes, comments or explanation should be included in /* ... */ blocks.
* Write a single implementation file.
* Implements the code in {language} for the following INPUT:

# INPUT:
{prompt_text}
"
                    ));
                }
                Language::Python => {
                    return Err(generic_error_str("NYI Python"));
                }
                _ => {
                    return Err(generic_error_str("Unknown Language"));
                }
            }
        }
    } else {
        generate_code_prompt = prompt.unwrap_or("".to_string());
    }

    if fetch_query {
        return Ok(json!({
            "query": generate_code_prompt,
        }));
    }

    let job_id = v2_create_and_send_job_message(
        bearer,
        job_creation_info,
        llm_provider,
        generate_code_prompt,
        db_clone,
        node_name_clone,
        identity_manager_clone,
        job_manager_clone,
        encryption_secret_key_clone,
        encryption_public_key_clone,
        signing_secret_key_clone,
    )
    .await?;

    Ok(json!({
        "job_id": job_id,
    }))
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
    // let tool_definitions = generate_tool_definitions(language.clone(), lance_db.clone(), true).await?;
    let mut generate_code_prompt = String::new();

    match language {
        Language::Typescript => {
            generate_code_prompt.push_str(&format!(
                "
# RULE I: 
These are two examples of METADATA:
## Example 1:
```json
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
```

## Example 2:
```json
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
```

# RULE II:
* Following the format of the examples provided.
* The METADATA must be in JSON format.
* Output only the METADATA, so the complete Output it's a valid JSON string.
* Any comments, notes, explanations or examples must be omitted in the Output.
* Generate the METADATA for the following {language} source code in the INPUT:

# INPUT:
```{language}
{}
```
",
                code.unwrap_or("".to_string())
            ));
        }
        Language::Python => {
            return Err(generic_error_str("NYI Python"));
        }
        _ => {
            return Err(generic_error_str("Unknown Language"));
        }
    }

    let job_id = v2_create_and_send_job_message(
        bearer,
        job_creation_info,
        llm_provider,
        generate_code_prompt,
        db_clone,
        node_name_clone,
        identity_manager_clone,
        job_manager_clone,
        encryption_secret_key_clone,
        encryption_public_key_clone,
        signing_secret_key_clone,
    )
    .await?;

    Ok(json!({
        "job_id": job_id,
    }))
}
