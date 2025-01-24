use serde_json::json;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::deno_tools::{DenoTool, ToolResult};
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::parameters::{Parameters, Property};
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
use std::sync::Arc;

pub async fn install_stagehand_processor(db: Arc<SqliteManager>) -> Result<(), ToolError> {
    let js_code = json!({
        "code": get_ts_code(),
        "package": get_ts_package(),
        "parameters": "<__PARAMETERS__>",
        "config": "<__CONFIG__>"
    });
    let js_code = js_code.to_string();
    let js_code = js_code.replace("\"<__PARAMETERS__>\"", "parameters");
    let js_code = js_code.replace("\"<__CONFIG__>\"", "config");

    let deno_tool = DenoTool {
        name: "Stagehand Processor".to_string(),
        homepage: None,
        description: "Tool for executing Node.js code in a sandboxed environment".to_string(),
        author: "@@official.shinkai".to_string(),
        version: "1.0.0".to_string(),
        input_args: {
            let mut params = Parameters::new();

            // Create the properties for the command object
            let mut command_props = std::collections::HashMap::new();
            command_props.insert(
                "id".to_string(),
                Property::new("string".to_string(), "Unique identifier for the command".to_string()),
            );

            // Create enum-like property for action
            let mut action_prop = Property::new(
                "string".to_string(),
                "Type of action to perform: 'goto' | 'wait' | 'evaluate' | 'act' | 'goto-stage' ".to_string(),
            );
            action_prop.property_type = "enum".to_string();
            command_props.insert("action".to_string(), action_prop);

            command_props.insert(
                        "payload".to_string(),
                        Property::new(
                            "string".to_string(),
                            "Action Payload: goto=>url, wait=>ms, evaluate=>text-prompt, act=>text-prompt, goto-stage=>stage-id"
                                .to_string(),
                        ),
                    );

            // Optional jsonSchema property
            command_props.insert(
                "jsonSchema".to_string(),
                Property::new(
                    "object".to_string(),
                    "Optional JSON schema for actions 'evaluate' and 'act'".to_string(),
                ),
            );

            // Create the command object property
            let command_object = Property::with_nested_properties(
                "object".to_string(),
                "A command to execute".to_string(),
                command_props,
            );

            // Create the commands array property
            let commands_array = Property::with_array_items("Array of commands to execute".to_string(), command_object);

            // Add the commands array to the parameters
            params.properties.insert("commands".to_string(), commands_array);
            params.required.push("commands".to_string());

            params
        },
        output_arg: ToolOutputArg {
            json: r#"{"type": "object", "properties": {"stdout": {"type": "string"}}}"#.to_string(),
        },
        js_code: format!(
            r#"
            import {{ shinkaiTypescriptUnsafeProcessor }} from "./shinkai-local-tools.ts";
            export async function run(config: any, inputs: any) {{
                return await shinkaiTypescriptUnsafeProcessor({js_code});
            }}
            "#
        ),
        tools: vec![ToolRouterKey::new(
            "local".to_string(),
            "@@official.shinkai".to_string(),
            "shinkai_typescript_unsafe_processor".to_string(),
            Some("1.0.0".to_string()),
        )],
        config: vec![],
        keywords: vec!["stagehand".to_string()],
        activated: true,
        embedding: None,
        result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
        sql_tables: None,
        sql_queries: None,
        file_inbox: None,
        oauth: None,
        assets: None,
    };
    let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);
    let _ = db
        .add_tool(shinkai_tool.clone())
        .await
        .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
    Ok(())
}

fn get_ts_code() -> String {
    let code = r#"

    import { jsonSchemaToZod } from "json-schema-to-zod";
    import pkg from '@browserbasehq/stagehand';
    const { Stagehand, ConstructorParams } = pkg;
    import { z } from "zod";
    z.object({});
    
    async function stagehandRun(config: CONFIG, inputs: INPUTS) {
        const stagehandConfig: ConstructorParams = {
            env: "LOCAL",
            modelName: "gpt-4o",
            modelClientOptions: {
                apiKey: "",
            },
            enableCaching: false,
            debugDom: true /* Enable DOM debugging features */,
            headless: false /* Run browser in headless mode */,
            domSettleTimeoutMs: 10_000 /* Timeout for DOM to settle in milliseconds */,
            verbose: 1,
        }
    
        console.log("🎮 Starting 2048 bot...");
        const stagehand = new Stagehand(stagehandConfig);
        try {
            console.log("🌟 Initializing Stagehand...");
            await stagehand.init();
            console.log("🌐 Navigating to 2048...");
            for (const input of inputs.commands) {
                switch (input.action) {
                    case "goto":
                        await stagehand.page.goto(input.payload);
                        break;
                    case "wait":
                        await new Promise((resolve) => setTimeout(resolve, parseInt(input.payload)));
                        break;
                    case "act":
                        await stagehand.page.act(input.payload);
                        break;
                    case "goto-stage":
                        await stagehand.gotoStage(input.payload);
                        break;
                }
            }
        } catch (error) {
            console.error("❌ Error", error);
            throw error; // Re-throw non-game-over errors
        }
    }
    
    
    const x = jsonSchemaToZod({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "score": {
                "type": "number"
            },
            "highestTile": {
                "type": "number"
            },
            "grid": {
                "type": "array",
                "items": {
                    "type": "array",
                    "items": {
                        "type": "number"
                    }
                }
            }
        },
        "required": ["score", "highestTile", "grid"],
        "additionalProperties": false
    });
    // console.log({ x, z: !!z.object({}) });
    const scoreSchema = eval(x);
    
    const moveSchema = eval(jsonSchemaToZod({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "move": {
                "type": "string",
                "enum": ["up", "down", "left", "right"]
            },
            "confidence": {
                "type": "number"
            },
            "reasoning": {
                "type": "string"
            }
        },
        "required": ["move", "confidence", "reasoning"],
        "additionalProperties": false
    }));
    
    type CONFIG = {};
    type INPUTS = {
        commands: {
            id: string,
            action: 'goto' | 'wait' | 'evaluate' | 'act' | 'goto-stage',
            payload: string,
            jsonSchema?: object
        }[]
    };
    
    type OUTPUTS = { message: string };
    ;
    
    async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUTS> {
        await stagehandRun(config, inputs);
        return { message: "OK" };
    }
"#;
    return code.to_string();
}

fn get_ts_package() -> String {
    let package = r#"
{
  "name": "standalone",
  "version": "1.0.0",
  "main": "index.ts",
  "scripts": {
    "test": "echo \"Error: no test specified\" && exit 1"
  },
  "author": "",
  "license": "ISC",
  "description": "",
  "dependencies": {
    "@browserbasehq/stagehand": "^1.10.0",
    "json-schema-to-zod": "^2.6.0",
    "zod": "^3.24.1"
  }
}
"#;
    return package.to_string();
}
