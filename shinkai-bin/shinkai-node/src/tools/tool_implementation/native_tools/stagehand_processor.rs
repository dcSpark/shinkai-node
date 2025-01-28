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
        description: "Tool for executing Stagehand (Browser Automation)".to_string(),
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
                "Type of action to perform: 'goto' | 'wait' | 'extract' | 'act' | 'goto-stage' ".to_string(),
            );
            action_prop.property_type = "enum".to_string();
            command_props.insert("action".to_string(), action_prop);

            command_props.insert(
                "payload".to_string(),
                Property::new(
                    "string".to_string(),
                    "Action Payload: goto=>url, wait=>ms, extract=>text-prompt, act=>text-prompt, goto-stage=>stage-id"
                        .to_string(),
                ),
            );

            // Optional jsonSchema property
            command_props.insert(
                "jsonSchema".to_string(),
                Property::new(
                    "object".to_string(),
                    "Optional JSON Schema for actions 'extract'".to_string(),
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
        tools: vec![
            ToolRouterKey::new(
                "local".to_string(),
                "@@official.shinkai".to_string(),
                "shinkai_typescript_unsafe_processor".to_string(),
                Some("1.0.0".to_string()),
            ),
            ToolRouterKey::new(
                "local".to_string(),
                "@@official.shinkai".to_string(),
                "shinkai_llm_prompt_processor".to_string(),
                Some("1.0.0".to_string()),
            ),
        ],
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
console.log(pkg);
const { Stagehand, ConstructorParams } = pkg;
import { z } from "zod";
z.object({});

async function shinkaiLlmPromptProcessor(query: { prompt: string }) {
    const response = await fetch(`${process.env.SHINKAI_NODE_LOCATION}/v2/tool_execution`, {
        method: "POST",
        headers: {
                'Authorization': `Bearer ${process.env.BEARER}`,
                'x-shinkai-tool-id': `${process.env.X_SHINKAI_TOOL_ID}`,
                'x-shinkai-app-id': `${process.env.X_SHINKAI_APP_ID}`,
                'x-shinkai-llm-provider': `${process.env.X_SHINKAI_LLM_PROVIDER}`
        },
        body: JSON.stringify({
            tool_router_key: "local:::__official_shinkai:::shinkai_llm_prompt_processor",
            llm_provider: `${process.env.X_SHINKAI_LLM_PROVIDER}`, 
            parameters: query
        })
    });

    if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
    }

    return await response.json();
}

async function getSchemaForExtract(prompt: string) {
    const llmPrompt = `
<rules>
* This command will be run when analyzing a webpage html.
* The user is requesting to extract some data from the page, and we need to generate a minimum json schema that can store these values.
* Prefer basic types as numbers, strings and boolean to store data.
* For the command in the input tag: generate a valid json schema that can store the properties requested in json format.
* write the json and nothing else, omit all comments, ideas or suggestions.
* print a valid json schema based on the template tag
</rules>

<template> 
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "example": {
      "type": "string" 
    }
  }
}
</template>

<input>
${prompt}
</input>
`;
    let reties = 3;
    while (true) {
        const schema = await shinkaiLlmPromptProcessor({ prompt: llmPrompt });
        const m = schema.message;
        // First try to extract data between ```json and ```
        const jsonData = m.match(/```json\s*([\s\S]*?)\s*```/);
        if (jsonData) {
            try { 
                return JSON.parse(jsonData[1]);
            } catch (error) {
                console.error("Error parsing JSON", error);
            }
        }
        try {
            return JSON.parse(m);
        } catch (error) {
            console.error("Error parsing JSON", error);
        }
        reties -= 1;
        if (reties < 1) throw new Error("Failed to generate schema");
    }
}
async function stagehandRun(config: CONFIG, inputs: INPUTS) {
    const stagehandConfig: ConstructorParams = {
        env: "LOCAL",
        modelName: "gpt-4o",
        modelClientOptions: {
            apiKey: "APIKEY",
        },
        enableCaching: false,
        debugDom: true /* Enable DOM debugging features */,
        headless: false /* Run browser in headless mode */,
        domSettleTimeoutMs: 10_000 /* Timeout for DOM to settle in milliseconds */,
        verbose: 1,
    }

    console.log("⭐ Starting Stagehand");
    const stagehand = new Stagehand(stagehandConfig);
    const data: any[] = []; 
    try {
        console.log("🌟 Initializing Stagehand...");
        await stagehand.init();
        let stage = 0;
        while (stage < inputs.commands.length) {
            const input = inputs.commands[stage];
            if (!input) break;
            switch (input.action) {
                case "goto":
                    console.log("🌐 Navigating to ", input.payload);
                    await stagehand.page.goto(input.payload);
                    stage++;
                    break;
                case "wait":
                    console.log("🕒 Waiting for ", input.payload, "ms");
                    await new Promise((resolve) => setTimeout(resolve, parseInt(input.payload)));
                    stage++;
                    break;
                case "act":
                    console.log("👋 Acting on ", input.payload);
                    await stagehand.page.act(input.payload);
                    stage++;
                    break;
                case "extract":
                    console.log("👋 Extract ", input.payload);
                    if (!input.jsonSchema) {
                        input.jsonSchema = await getSchemaForExtract(input.payload);
                    }
                    const z_schema = jsonSchemaToZod(input.jsonSchema);
                    console.log(z_schema);
                    const schema = eval(z_schema);
                    const result: any = await stagehand.page.extract({ instruction: input.payload, schema });
                    data.push(result);
                    stage++;
                    break;
                case "goto-stage":
                    console.log("🔗 Going to stage ", input.payload);
                    const stageIndex = inputs.commands.findIndex(cmd => cmd.id === input.payload);
                    if (stageIndex === -1) {
                        throw new Error("Stage not found");
                    }
                    if (stage === stageIndex) throw new Error("Stage already reached");
                    stage = stageIndex;
                    break;
                default:
                    throw new Error("Invalid action");
            }
        }
    } catch (error) {
        try {
            await stagehand.close();
        } catch (error) {
            console.error("❌ Cannot close stagehand", error);
        }
        console.error("❌ Error", error);
        throw error; // Re-throw non-game-over errors
    }
    await stagehand.close();
    return data;
}

type CONFIG = {};
type INPUTS = {
    commands: {
        id: string,
        action: 'goto' | 'wait' | 'act' | 'goto-stage' | 'extract',
        payload: string,
        jsonSchema?: object
    }[]
};

type OUTPUTS = { message: string, data: any[] };
;

async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUTS> {
    const data = await stagehandRun(config, inputs);
    return { message: "success", data: data };
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
        "@browserbasehq/stagehand": "https://github.com/dcSpark/stagehand",
        "json-schema-to-zod": "^2.6.0",
        "sharp": "^0.33.5",
        "zod": "^3.24.1"
    }
}

"#;
    return package.to_string();
}
