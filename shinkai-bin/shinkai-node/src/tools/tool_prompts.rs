use crate::managers::IdentityManager;
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::{shinkai_tools::CodeLanguage, tool_router_key::ToolRouterKey};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

pub async fn generate_code_prompt(
    language: CodeLanguage,
    is_memory_required: bool,
    prompt: String,
    support_files: HashMap<String, String>,
) -> Result<String, APIError> {
    match language {
        CodeLanguage::Typescript => {
            let shinkai_sqlite_query_executor = "shinkaiSqliteQueryExecutor";
            let is_memory_required_message = if is_memory_required {
                format!("* If permanent memory is required, write to disk, store, sql always prioritize using {shinkai_sqlite_query_executor}.")
            } else {
                "".to_string()
            };
            let ts_support_files_section = support_files
                .iter()
                .map(|(name, content)| {
                    format!(
                        "Import these functions with the format: `import {{ xx }} from './{name}.ts'                   
  <file-name={name}>
```{language}
{content}
```
  </file-name={name}>
"
                    )
                })
                .collect::<Vec<String>>()
                .join("\n");

            return Ok(format!(
                r#"
<agent_libraries>
  * You may use any of the following functions if they are relevant and a good match for the task.
  * These are the libraries available in the same directory:

  {ts_support_files_section}
</agent_libraries>

<agent_deno_libraries>
  * Prefer libraries in the following order:
    1. A function provided by './shinkai-local-tools.ts' that resolves correctly the requirement.
    2. If fetch is required, it is available in the global scope without any import.
    3. The code will be ran with Deno Runtime, so prefer Deno default and standard libraries.
    4. If an external system has a well known and defined API, prefer to call the API instead of downloading a library.
    5. If an external system requires to be used through a package (Deno, Node or NPM), or the API is unknown the NPM library may be used with the 'npm:' prefix.
  * If OAuth is required, use the 'getAccessToken' function to get a valid OAuth AccessToken for the given provider.
</agent_deno_libraries>

<agent_code_format>
  * To implement the task you can update the CONFIG, INPUTS and OUTPUT types to match the run function type:
  ```{language}
    type CONFIG = {{}};
    type INPUTS = {{}};
    type OUTPUT = {{}};
    export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {{
        return {{}};
    }}
  ```
  * CONFIG, INPUTS and OUTPUT must be objects, not arrays neither basic types.
</agent_code_format>

<agent_code_rules>
  * All import must be in the beginning of the file. Do not use dynamic imports.
  * If "Buffer" is used, then import it with `import {{ Buffer }} from 'node:buffer';`
  * The code will be shared as a library, when used it run(...) function will be called.
  * The function signature MUST be: `export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT>`
  {is_memory_required_message}
</agent_code_rules>

<agent_code_implementation>
  * Do not output, notes, ideas, explanations or examples.
  * Write only valid {language} code, so the complete printed code can be directly executed.
  * Only if required any additional notes, comments or explanation should be included in /* ... */ blocks.
  * Write a single implementation file, only one typescript code block.
  * Implements the code in {language} for the following input_command tag.
</agent_code_implementation>

<agent_libraries_documentation>
  <deno>
    Native Deno Library to Write Files `Deno.writeFile(path, data, options)`
    This function is available in the global scope without any import.
    The home path for files is available in through the `getHomePath()` function.
    ```typescript
      Deno.writeFile(
        path: string | URL,
        data: Uint8Array | ReadableStream<Uint8Array>,
        options?: WriteFileOptions,
      ): Promise<void>
    ```
    Examples:
    ```typescript
      await Deno.writeFile(`${{getHomePath()}}/hello1.txt`, new TextEncoder().encode("Hello world\n")); 
      await Deno.writeFile(`${{getHomePath()}}/image.png`, data);
    ```
  </deno>
</agent_libraries_documentation>

<input_command>
{prompt}
</input_command>

Explain your thinking process step by step and then implement the code.

"#
            ));
        }
        CodeLanguage::Python => {
            let shinkai_sqlite_query_executor = "shinkaiSqliteQueryExecutor";
            let is_memory_required_message = if is_memory_required {
                format!("* If permanent memory is required, write to disk, store, sql always prioritize using {shinkai_sqlite_query_executor}.")
            } else {
                "".to_string()
            };
            let py_support_files_section = support_files
                .iter()
                .map(|(name, content)| {
                    format!(
                        "Import these functions with the format: `from {name} import xx`                  
  <file-name={name}>
```{language}
{content}
```
  </file-name={name}>
"
                    )
                })
                .collect::<Vec<String>>()
                .join("\n");

            return Ok(format!(
                r#"
<agent_libraries>
  * You may use any of the following functions if they are relevant and a good match for the task.
  * Import them with the format: `from shinkai_local_tools import xx`
  
  {py_support_files_section}
</agent_libraries>

<agent_python_libraries>
* Prefer libraries in the following order:
  1. A function provided by './shinkai_local_tools.py' that resolves correctly the requirement.
  2. If network fetch is required, use the "requests" library and import it with using `import requests`.
  3. The code will be ran with Python Runtime, so prefer Python default and standard libraries. Import all used libraries as `from <library> import <function>` for example for Lists use `from typing import List`.
  4. If an external system requires to be used through a package, or the API is unknown use "pip" libraries.
  5. If an external system has a well known and defined API, call the API endpoints.
* If OAuth is required, use the 'get_access_token' function to get a valid OAuth AccessToken for the given provider.
</agent_python_libraries>

<agent_code_format>
  * To implement the task you can update the CONFIG, INPUTS and OUTPUT types to match the run function type:
  ```{language}
# /// script
# requires-python = ">=3.10,<3.12"
# dependencies = [
#   "requests",
# ]
# ///

from typing import Any, Optional, List, Dict

class CONFIG:
    pass

class INPUTS:
    pass

class OUTPUT:
    pass

async def run(config: CONFIG, inputs: INPUTS) -> OUTPUT:
    output = Output()
    return output
  ```
  * Update CONFIG, INPUTS and OUTPUT as follows but with the correct members to correcly implement the input_command tag.
```{language}
class CONFIG:
    arg1: str
    arg2: int
    arg3: List[str]

class INPUTS:
    sample: List[str]
    argN: Optional[str] = None

class OUTPUT:
    complex_output: List[dict]
    another_sample: str
```

</agent_code_format>

<agent_code_rules>
  * Do not implement __init__ or __new__ methods for CONFIG, INPUTS or OUTPUT.
  * The code will be shared as a library, when used it run(...) function will be called.
  * The function signature MUST be: `async def run(config: CONFIG, inputs: INPUTS) -> OUTPUT`
  {is_memory_required_message}
</agent_code_rules>

<agent_code_implementation>
  * Do not output, notes, ideas, explanations or examples.
  * Write only valid {language} code, so the complete printed code can be directly executed.
  * Only if required any additional notes, comments or explanation lines should be prefixed with # character.
  * Write a single implementation file, only one typescript code block.
  * Implements the code in {language} for the following input_command tag
</agent_code_implementation>

<agent_pip_requirements>
  * At the start of the file add a commented toml code block with the dependencies used and required to be downloaded by pip.
  * Only add the dependencies that are required to be downloaded by pip, do not add the dependencies that are already available in the Python environment.
  * This is an example of the commented script block that MUST be present before any python code or imports.

# /// script
# requires-python = ">=3.10,<3.12"
# dependencies = [
#   "requests",
#   "ruff >=0.3.0",
#   "torch ==2.2.2",
#   "other_dependency",
#   "other_dependency_2",
# ]
# ///

  * Always add "requests" to the dependencies list.

</agent_pip_requirements>

<input_command>
{prompt}
</input_command>

Explain your thinking process step by step and then implement the code.

"#
            ));
        }
    }
}

pub async fn tool_metadata_implementation_prompt(
    language: CodeLanguage,
    code: String,
    tools: Vec<ToolRouterKey>,
    identity_manager: Arc<Mutex<IdentityManager>>,
) -> Result<String, APIError> {
    let identity_manager = identity_manager.lock().await;
    let identity_name = identity_manager.local_node_name.to_string();
    drop(identity_manager);
    // code might be json string as {
    //  "job_id":"jobid_c7c5c9f5-e3a3-4667-ba67-e8b838c2f5db",
    //  "content":"```typescript\ ..console.log.. ```",
    //  "files_inbox":"",
    //  "parent":null,
    //  "sheet_job_data":null,
    //  "callback":null,
    //  "metadata":{"tps":null,"duration_ms":"2824","function_calls":[]},
    // "tool_key":null}
    // we need to extract the code from the json string
    let json = serde_json::from_str::<serde_json::Value>(&code);
    let mut final_code = code;
    if let Ok(json) = json {
        let code = json.get("content");
        if let Some(code) = code {
            final_code = code.to_string();
        }
    }

    let has_oauth = (language == CodeLanguage::Typescript && final_code.contains("getAccessToken("))
        || (language == CodeLanguage::Python && final_code.contains("get_access_token("));
    let oauth_example = if has_oauth {
        r#"[
      {{
        "name": "google",
        "version": "2.0",
        "authorizationUrl": "https://accounts.google.com/o/oauth2/v2/auth",
        "redirectUrl": "https://secrets.shinkai.com/redirect",
        "tokenUrl": "https://oauth2.googleapis.com/token",
        "clientId": "YOUR_PROVIDER_CLIENT_ID",
        "clientSecret": "YOUR_PROVIDER_CLIENT_SECRET",
        "scopes": [
          "https://www.googleapis.com/auth/userinfo.email",
          "https://www.googleapis.com/auth/userinfo.profile"
        ],
        "response_type": "code"
      }}
    ]"#
    } else {
        r#"[]"#
    };
    let oauth_explain = if has_oauth {
        r#"
  * OAuth is required. For each get_access_token or getAccessToken in the input_command tag you must provide an OAuth configuration.
  * getAccessToken(name) must match the metadata oauth name field.
  * OAuth version 1.0 or 2.0 is supported, if possible prefer 1.0 over 2.0.
  * Leave refreshToken and accessToken empty, they will be filled later on.
"#
    } else {
        r#""#
    };

    let oauth_template = if has_oauth {
        r#",
      oauth": [
        {{
          "name": "",
          "version": "",
          "authorizationUrl": "",
          "redirectUrl": "",
          "tokenUrl": "",
          "clientId": "",
          "clientSecret": "",
          "scopes": [],
        }}
      ]
    "#
    } else {
        r#""#
    };

    let empty_template = format!(
        r#"
  {{
      "name": "<name>",
      "homepage": "<url>",
      "description": "<description>",
      "author": "{identity_name}",
      "version": "1.0.0",
      "keywords": [],
      "runner": "any",
      "operatingSystem": ["linux", "macos", "windows"],
      "tool_set": "",
      "configurations": {{
        "type": "object",
        "properties": {{}},
        "required": []
      }},
      "parameters": {{
        "type": "object",
        "properties": {{}}
        "required": []
      }},
      "result": {{
        "type": "object",
        "properties": {{}}
        "required": []
      }},
      "sqlTables": [],
      "sqlQueries": [],
      "tools": []{}
      
  }}
"#,
        oauth_template
    );

    Ok(format!(
        r####"
<agent_metadata_schema>
  * This is the SCHEMA for the METADATA:
  ```json
    {{
    "name": "function",
    "schema": {{
      "type": "object",
      "properties": {{
        "name": {{
          "type": "string",
          "description": "The name of the function"
        }},
        "homepage": {{
          "type": "string",
          "description": "URL of the homepage"
        }},
        "description": {{
          "type": "string",
          "description": "A description of what the function does"
        }},
        "version": {{
          "type": "string",
          "description": "The version of the function"
        }},
        "author": {{
          "type": "string",
          "description": "The author of the function"
        }},
        "keywords": {{
          "type": "array",
          "description": "A list of keywords that describe the function",
          "items": {{
            "type": "string"
          }}
        }},
        "runner": {{
          "type": "string",
          "enum": ["any", "only_host", "only_docker"],
          "description": "The type of runner required for this tool"
        }},
        "operatingSystem": {{
          "type": "array",
          "description": "List of supported operating systems",
          "items": {{
            "type": "string",
            "enum": ["linux", "macos", "windows"]
          }}
        }},
        "tool_set": {{
          "type": "string",
          "description": "Optional Tool Set identifier"
        }},
        "configurations": {{
          "$ref": "#/$defs/root_type",
          "description": "A JSON schema that defines the function's configurations"
        }},
        "parameters": {{
          "$ref": "#/$defs/root_type",
          "description": "A JSON schema that defines the function's parameters"
        }},
        "result": {{
          "$ref": "#/$defs/root_type",
          "description": "A JSON schema that defines the function's result"
        }},
        "sqlTables": {{
          "type": "array",
          "description": "A list of SQL tables used by the function",
          "items": {{
            "type": "object",
            "properties": {{
              "name": {{
                "type": "string",
                "description": "The unique name of the SQL table."
              }},
              "definition": {{
                "type": "string",
                "description": "The SQL Query to create the table"
              }},
            }},
            "required": ["name", "definition"]
          }}
        }},
        "sqlQueries": {{
          "type": "array",
          "description": "A list of SQL queries used by the function",
          "items": {{
            "type": "object",
            "properties": {{
              "name": {{
                "type": "string",
                "description": "A functional name to describe the SQL query."
              }},
              "query": {{
                "type": "string",
                "description": "The SQL query to retrieve data from the database."
              }},
            }},
            "required": ["name", "query"]
          }}
        }},
        "tools": {{
          "type": "array",
          "description": "A list of tools used by the function",
          "items": {{
            "type": "string"
          }}
        }},
        "oauth": {{
          "type": "array",
          "description": "A list of OAuth integrations",
          "items": {{
            "type": "object",
            "properties": {{
              "name": {{
                "type": "string",
                "description": "The unique name of the OAuth integration."
              }},
              "version": {{
                "type": "string",
                "description": "The version of the OAuth integration: 1.0 or 2.0."
              }},
              "authorizationUrl": {{
                "type": "string",
                "format": "uri",
                "description": "The endpoint to obtain authorization from the resource owner."
              }},
              "redirectUrl": {{
                "type": "string",
                "format": "uri",
                "description": "The redirect URI for the OAuth integration.",
                "default": "https://secrets.shinkai.com/redirect"
              }},
              "responseType": {{
                "type": "string",
                "description": "The OAuth 2.0 response type (e.g., 'token').",
                "default": "token"
              }},
              "tokenUrl": {{
                "type": "string",
                "format": "uri",
                "description": "The endpoint to exchange the authorization grant for an access token."
              }},
              "clientId": {{
                "type": "string",
                "description": "The client identifier issued to the client during registration."
              }},
              "clientSecret": {{
                "type": "string",
                "description": "The client secret issued during registration."
              }},
              "scopes": {{
                "type": "array",
                "description": "A list of scopes required for the integration.",
                "items": {{
                  "type": "string"
                }}
              }},
              "response_type": {{
                "type": "string",
                "description": "OAuth response_type parameter.",
                "default": "code"
              }}
            }},
            "required": [
              "authorizationUrl",
              "clientId",
              "clientSecret"
            ],
            "additionalProperties": false
          }}
        }}
      }},
      "required": [
        "name",
        "description",
        "author",
        "keywords",
        "runner",
        "operatingSystem",
        "configurations",
        "parameters",
        "result",
        "sqlTables",
        "sqlQueries",
        "tools",
        "oauth"
      ],
      "additionalProperties": false
    }},
    "$defs": {{
      "root_type": {{
        "properties": {{
          "type": "object",
          "additionalProperties": {{
            "$ref": "#/$defs/schema_definition"
          }}
        }}
      }},
      "schema_definition": {{
        "type": "object",
        "properties": {{
          "name": {{
            "type": "string"
          }},
          "description": {{
            "type": "string"
          }},
          "order": {{
            "type": "number"
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
          "type",
          "name",
          "description",
          "order"
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
        }}
      }}
    }}
  }}
  ```
</agent_metadata_schema>
<agent_metadata_examples>
  These are two examples of METADATA:
  ## Example 1:
  Output: ```json
  {{
    "name": "Coinbase Wallet Creator",
    "homepage": "https://shinkai.com",
    "description": "Tool for creating a Coinbase wallet",
    "author": "{identity_name}",
    "version": "1.0.0",
    "keywords": [
      "coinbase",
      "wallet",
      "creator",
      "shinkai"
    ],
    "runner": "any",
    "operatingSystem": ["linux", "macos", "windows"],
    "tool_set": "",
    "configurations": {{
      "type": "object",
      "properties": {{
        "name": {{ "type": "string", "description": "The name of the function" }},
        "privateKey": {{ "type": "string", "description": "The private key of the function" }},
        "useServerSigner": {{ "type": "string", "default": "false", "nullable": true, "description": "Whether to use the server signer" }},
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
        "walletId": {{ "type": "string", "description": "The ID of the wallet" }},
        "seed": {{ "type": "string", "description": "The seed of the wallet" }},
        "address": {{ "type": "string", "description": "The address of the wallet" }},
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
    ],
    "tools": [
      "local:::__official_shinkai:::shinkai_sqlite_query_executor",
      "local:::shinkai_tool_echo:::shinkai_echo"
    ],
    "oauth": {oauth_example},
    "runner": "any",
    "operatingSystem": ["linux", "macos", "windows"],
    "tool_set": ""
  }};
  ```

  ## Example 2:
  Output:```json
  {{
    "name": "Download Pages",
    "homepage": "https://shinkai.com",
    "description": "Downloads one or more URLs and sends the html content as markdown to an email address.",
    "author": "{identity_name}",
    "version": "1.0.0",
    "keywords": [
      "HTML to Markdown",
      "web page downloader",
      "content conversion",
      "URL to Markdown",
    ],
    "runner": "any",
    "operatingSystem": ["linux", "macos", "windows"],
    "tool_set": "",
    "configurations": {{
      "type": "object",
      "properties": {{}},
      "required": []
    }},
    "parameters": {{
      "type": "object",
      "properties": {{
        "urls": {{ "type": "array", "description": "The URLs to download", "items": {{ "type": "string", "description": "URL to download" }} }},
        "email": {{ "type": "string", "description": "The email to send the markdown to" }},
        "subject": {{ "type": "string", "description": "The subject of the email" }},
      }},
      "required": [
        "urls"
      ]
    }},
    "result": {{
      "type": "object",
      "properties": {{
        "markdowns": {{ "type": "array", "items": {{ "type": "string", "description": "markdown content" }}, "description": "The markdown content of the downloaded pages" }},
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
    ],
    "tools": [],
    "oauth": {oauth_example},
    "runner": "any",
    "operatingSystem": ["linux", "macos", "windows"],
    "tool_set": ""
  }};
  ```
</agent_metadata_examples>

<agent_metadata_rules>
  * If the code uses shinkaiSqliteQueryExecutor then fill the sqlTables and sqlQueries sections, otherwise these sections are empty.
  * sqlTables contains the complete table structures, they should be same as in the code.
  * sqlQueries contains from 1 to 3 examples that show how the data should be retrieved for usage.
  * If properties from CONFIG have default values, then those properties shouldn't be in the required array.
{oauth_explain}
</agent_metadata_rules>

<available_tools>
{}
</available_tools>

<agent_metadata_implementation>
  * Return a valid schema for the described JSON, remove trailing commas.
  * The METADATA must be in JSON valid format in only one JSON code block and nothing else.
  * Output only the METADATA, so the complete Output it's a valid JSON string.
  * Any comments, notes, explanations or examples must be omitted in the Output.
  * Use the available_tools section to get the list of tools for the metadata.
  * Generate the METADATA for the following source code in the input_command tag.
  * configuration, parameters and result must be objects, not arrays neither basic types.
  * Create the schema starting by using the empty_template tag
</agent_metadata_implementation>

<input_command>
{}
</input_command>

<empty_template>
{}
</empty_template>

"####,
        tools
            .iter()
            .map(|tool: &ToolRouterKey| tool.to_string_without_version())
            .collect::<Vec<String>>()
            .join("\n"),
        final_code,
        empty_template
    ))
}
