use ed25519_dalek::SigningKey;
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use std::sync::Arc;
use tokio::sync::Mutex;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::managers::IdentityManager;
use crate::{llm_provider::job_manager::JobManager, network::Node};

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

    let job_id = res_receiver
        .recv()
        .await
        .map_err(|e| Node::generic_api_error(&e.to_string()))??;

    // Use the new function to send the message
    v2_send_basic_job_message_for_existing_job(
        bearer,
        job_id.clone(),
        content,
        db_clone,
        node_name_clone,
        identity_manager_clone,
        job_manager_clone,
        encryption_secret_key_clone,
        encryption_public_key_clone,
        signing_secret_key_clone,
    )
    .await?;

    Ok(job_id)
}

pub async fn v2_send_basic_job_message_for_existing_job(
    bearer: String,
    job_id: String,
    content: String,
    db_clone: Arc<ShinkaiDB>,
    node_name_clone: ShinkaiName,
    identity_manager_clone: Arc<Mutex<IdentityManager>>,
    job_manager_clone: Arc<Mutex<JobManager>>,
    encryption_secret_key_clone: EncryptionStaticKey,
    encryption_public_key_clone: EncryptionPublicKey,
    signing_secret_key_clone: SigningKey,
) -> Result<(), APIError> {
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

    res_receiver
        .recv()
        .await
        .map_err(|e| Node::generic_api_error(&e.to_string()))??;
    Ok(())
}

pub async fn generate_code_prompt(
    language: CodeLanguage,
    prompt: String,
    tool_definitions: String,
) -> Result<String, APIError> {
    match language {
        CodeLanguage::Typescript => {
            return Ok(format!(
                r####"
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
* If permanent memory is required, write to disk, store, sql always prioritize using shinkaiSqliteQueryExecutor.

# RULE IV:
* Do not output, notes, ideas, explanations or examples.
* Output only valid {language} code, so the complete Output can be directly executed.
* Only if required any additional notes, comments or explanation should be included in /* ... */ blocks.
* Write a single implementation file, only one typescript code block.
* Implements the code in {language} for the following INPUT:

{prompt}
"####
            ));
        }
        CodeLanguage::Python => {
            return Err(Node::generic_api_error("NYI Python"));
        }
    }
}

// TODO: move to commands_tools as an endpoint implementation
// pub async fn generate_tool_fetch_query(
//     language: CodeLanguage,
//     tool_definitions: String,
// ) -> Result<String, APIError> {
//     Ok(generate_code_prompt(language, "".to_string(), tool_definitions).await?)
// }

pub async fn tool_metadata_implementation(language: CodeLanguage, code: String) -> Result<String, APIError> {
    // Generate tool definitions first
    // let tool_definitions = generate_tool_definitions(language.clone(), sqlite_manager.clone(), true).await?;

    let mut generate_code_prompt = String::new();

    match language {
        CodeLanguage::Typescript => {
            generate_code_prompt.push_str(&format!(
                r####"
# RULE I:
This is the SCHEMA for the METADATA:
```json
 {{
  "name": "metaschema",
  "schema": {{
    "type": "object",
    "properties": {{
      "name": {{
        "type": "string",
        "description": "The name of the schema"
      }},
      "type": {{
        "type": "string",
        "enum": [
          "object",
          "array",
          "string",
          "number",
          "boolean",
          "null"
        ]
      }},
      "properties": {{
        "type": "object",
        "additionalProperties": {{
          "$ref": "#/$defs/schema_definition"
        }}
      }},
      "items": {{
        "anyOf": [
          {{
            "$ref": "#/$defs/schema_definition"
          }},
          {{
            "type": "array",
            "items": {{
              "$ref": "#/$defs/schema_definition"
            }}
          }}
        ]
      }},
      "required": {{
        "type": "array",
        "items": {{
          "type": "string"
        }}
      }},
      "additionalProperties": {{
        "type": "boolean"
      }}
    }},
    "required": [
      "type"
    ],
    "additionalProperties": false,
    "if": {{
      "properties": {{
        "type": {{
          "const": "object"
        }}
      }}
    }},
    "then": {{
      "required": [
        "properties"
      ]
    }},
    "$defs": {{
      "schema_definition": {{
        "type": "object",
        "properties": {{
          "type": {{
            "type": "string",
            "enum": [
              "object",
              "array",
              "string",
              "number",
              "boolean",
              "null"
            ]
          }},
          "properties": {{
            "type": "object",
            "additionalProperties": {{
              "$ref": "#/$defs/schema_definition"
            }}
          }},
          "items": {{
            "anyOf": [
              {{
                "$ref": "#/$defs/schema_definition"
              }},
              {{
                "type": "array",
                "items": {{
                  "$ref": "#/$defs/schema_definition"
                }}
              }}
            ]
          }},
          "required": {{
            "type": "array",
            "items": {{
              "type": "string"
            }}
          }},
          "additionalProperties": {{
            "type": "boolean"
          }}
        }},
        "required": [
          "type"
        ],
        "additionalProperties": false,
        "sqlTables": {{
          "type": "array",
          "items": {{
            "type": "object",
            "properties": {{
              "name": {{
                "type": "string",
                "description": "Name of the table"
              }},
              "definition": {{
                "type": "string",
                "description": "SQL CREATE TABLE statement"
              }}
            }},
            "required": ["name", "definition"]
          }}
        }},
        "sqlQueries": {{
          "type": "array",
          "items": {{
            "type": "object",
            "properties": {{
              "name": {{
                "type": "string",
                "description": "Name/description of the query"
              }},
              "query": {{
                "type": "string",
                "description": "Example SQL query"
              }}
            }},
            "required": ["name", "query"]
          }}
        }}
        "if": {{
          "properties": {{
            "type": {{
              "const": "object"
            }}
          }}
        }},
        "then": {{
          "required": [
            "properties"
          ]
        }}
      }}
    }}
  }}
}}
```

These are two examples of METADATA:
## Example 1:
Output: ```json
{{
  "id": "shinkai-tool-coinbase-create-wallet",
  "name": "Shinkai: Coinbase Wallet Creator",
  "description": "Tool for creating a Coinbase wallet",
  "author": "Shinkai",
  "keywords": [
    "coinbase",
    "wallet",
    "creator",
    "shinkai"
  ],
  "configurations": {{
    "type": "object",
    "properties": {{
      "name": {{ "type": "string" }},
      "privateKey": {{ "type": "string" }},
      "useServerSigner": {{ "type": "string", "default": "false", "nullable": true }},
    }},
    "required": [
      "name",
      "privateKey"
    ]
  }},
  "parameters": {{
    "type": "object",
    "properties": {{}},
    "required": []
  }},
  "result": {{
    "type": "object",
    "properties": {{
      "walletId": {{ "type": "string", "nullable": true }},
      "seed": {{ "type": "string", "nullable": true }},
      "address": {{ "type": "string", "nullable": true }},
    }},
    "required": []
  }},
  "sqlTables": [
    {{
      "name": "wallets",
      "definition": "CREATE TABLE wallets (id VARCHAR(255) PRIMARY KEY, name VARCHAR(255) NOT NULL, private_key TEXT NOT NULL, address VARCHAR(255), created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)"
    }}
  ],
  "sqlQueries": [
    {{
      "name": "Get wallet by address",
      "query": "SELECT * FROM wallets WHERE address = :address"
    }}
  ]
}};
```

## Example 2:
Output:```json
{{
  "id": "shinkai-tool-download-pages",
  "name": "Shinkai: Download Pages",
  "description": "Downloads one or more URLs and converts their HTML content to Markdown",
  "author": "Shinkai",
  "keywords": [
    "HTML to Markdown",
    "web page downloader",
    "content conversion",
    "URL to Markdown",
  ],
  "configurations": {{
    "type": "object",
    "properties": {{}},
    "required": []
  }},
  "parameters": {{
    "type": "object",
    "properties": {{
      "urls": {{ "type": "array", "items": {{ "type": "string" }} }},
    }},
    "required": [
      "urls"
    ]
  }},
  "result": {{
    "type": "object",
    "properties": {{
      "markdowns": {{ "type": "array", "items": {{ "type": "string" }} }},
    }},
    "required": [
      "markdowns"
    ]
  }},
  "sqlTables": [
    {{
      "name": "downloaded_pages",
      "definition": "CREATE TABLE downloaded_pages (id SERIAL PRIMARY KEY, url TEXT NOT NULL, markdown_content TEXT, downloaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)"
    }}
  ],
  "sqlQueries": [
    {{
      "name": "Get page by URL",
      "query": "SELECT * FROM downloaded_pages WHERE url = :url ORDER BY downloaded_at DESC LIMIT 1"
    }}
  ]
}};
```

# RULE II:
If the code uses shinkaiSqliteQueryExecutor then fill the sqlTables and sqlQueries sections, otherwise these sections are empty.
sqlTables contains the complete table structures, they should be same as in the code.
sqlQueries contains from 1 to 3 examples that show how the data should be retrieved for usage.

# RULE III:
* Return a valid schema for the described JSON, remove trailing commas.
* The METADATA must be in JSON valid format in only one JSON code block and nothing else.
* Output only the METADATA, so the complete Output it's a valid JSON string.
* Any comments, notes, explanations or examples must be omitted in the Output.
* Generate the METADATA for the following source code in the INPUT:

# INPUT:
{}
"####,
                code.clone()
            ));
            return Ok(generate_code_prompt);
        }
        CodeLanguage::Python => {
            return Err(Node::generic_api_error("NYI Python"));
        }
    }
}
