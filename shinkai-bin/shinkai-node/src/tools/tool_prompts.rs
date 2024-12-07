use std::collections::HashMap;

use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;

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
# <{name}>
```{language}
{content}
```
  </{name}>
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
    1. A function provided by './shinkai-local-tools.ts' that resolves correctly the requierement.
    2. If fetch is required, it is available in the global scope without any import.
    3. The code will be ran with Deno Runtime, so prefer Deno default and standard libraries.
    4. If an external system has a well known and defined API, prefer to call the API instead of downloading a library.
    5. If an external system requires to be used through a package (Deno, Node or NPM), or the API is unknown the NPM library may be used with the 'npm:' prefix.
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
# <{name}>
```{language}
{content}
```
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
  1. A function provided by './shinkai_local_tools.py' that resolves correctly the requierement.
  2. If network fetch is required, use the "requests" library and import it with using `import requests`.
  3. The code will be ran with Python Runtime, so prefer Python default and standard libraries. Import all used libraries as `from <library> import <function>` for example for Lists use `from typing import List`.
  4. If an external system requires to be used through a package, or the API is unknown use "pip" libraries.
  5. If an external system has a well known and defined API, call the API endpoints.
</agent_python_libraries>

<agent_code_format>
  * To implement the task you can update the CONFIG, INPUTS and OUTPUT types to match the run function type:
  ```{language}
from typing import Dict, Any, Optional, List

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

class OUTPUT:
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

<input_command>
{prompt}
</input_command>

"#
            ));
        }
    }
}

pub async fn tool_metadata_implementation_prompt(
    _language: CodeLanguage,
    code: String,
    tools: Vec<String>,
) -> Result<String, APIError> {
    Ok(format!(
        r####"
<agent_metadata_schema>
  * This is the SCHEMA for the METADATA:
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
          }},
          "tools": {{
            "type": "array",
            "items": {{
              "type": "string"
            }}
          }},
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
</agent_metadata_schema>
<agent_metadata_examples>
  These are two examples of METADATA:
  ## Example 1:
  Output: ```json
  {{
    "id": "coinbase-create-wallet",
    "name": "Coinbase Wallet Creator",
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
    ],
    "tools": [
      "local:::rust_toolkit:::shinkai_sqlite_query_executor",
      "local:::shinkai_tool_echo:::shinkai_echo"
    ]
  }};
  ```

  ## Example 2:
  Output:```json
  {{
    "id": "tool-download-pages",
    "name": "Download Pages",
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
    ],
    "tools": []
  }};
  ```
</agent_metadata_examples>

<agent_metadata_rules>
  * If the code uses shinkaiSqliteQueryExecutor then fill the sqlTables and sqlQueries sections, otherwise these sections are empty.
  * sqlTables contains the complete table structures, they should be same as in the code.
  * sqlQueries contains from 1 to 3 examples that show how the data should be retrieved for usage.
</agent_metadata_rules>

<available_tools>
{:?}
</available_tools>

<agent_metadata_implementation>
  * Return a valid schema for the described JSON, remove trailing commas.
  * The METADATA must be in JSON valid format in only one JSON code block and nothing else.
  * Output only the METADATA, so the complete Output it's a valid JSON string.
  * Any comments, notes, explanations or examples must be omitted in the Output.
  * Use the available_tools section to get the list of tools for the metadata.
  * Generate the METADATA for the following source code in the input_command tag.
  * configuration, parameters and result must be objects, not arrays neither basic types.
</agent_metadata_implementation>

<input_command>
{}
</input_command>

"####,
        tools,
        code.clone()
    ))
}
