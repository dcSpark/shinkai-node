use serde_json::{Map, Value};
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::js_tools::{JSTool, JSToolResult};
use shinkai_tools_primitives::tools::argument::ToolArgument;

pub fn execute_deno_tool(
    tool_router_key: String,
    parameters: Map<String, Value>,
    extra_config: Option<String>,
) -> Result<Value, ToolError> {
    // Extract the JavaScript code from parameters
    let js_code = parameters
        .get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::ExecutionError("Missing 'code' parameter".to_string()))?
        .to_string();

    let code = format!("\"use strict\";
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {{
  for (var name in all)
    __defProp(target, name, {{ get: all[name], enumerable: true }});
}};
var __copyProps = (to, from, except, desc) => {{
  if (from && typeof from === \"object\" || typeof from === \"function\") {{
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, {{ get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable }});
  }}
  return to;
}};
var __toCommonJS = (mod) => __copyProps(__defProp({{}}, \"__esModule\", {{ value: true }}), mod);

// apps/shinkai-tool-foobar/src/index.ts
var src_exports = {{}};
__export(src_exports, {{
  Tool: () => Tool
}});
module.exports = __toCommonJS(src_exports);

// libs/shinkai-tools-builder/src/base-tool.ts
var BaseTool = class {{
  config;
  constructor(config) {{
    this.config = config;
  }}
  getDefinition() {{
    return this.definition;
  }}
  setConfig(value) {{
    this.config = value;
    return this.config;
  }}
  getConfig() {{
    return this.config;
  }}
}};

// apps/shinkai-tool-foobar/src/index.ts
var Tool = class extends BaseTool {{
  definition = {{
    id: \"shinkai-tool-foobar\",
    name: \"Shinkai: foobar\",
    description: \"New foobar tool from template\",
    author: \"Shinkai\",
    keywords: [\"foobar\", \"shinkai\"],
    configurations: {{
      type: \"object\",
      properties: {{}},
      required: []
    }},
    parameters: {{
      type: \"object\",
      properties: {{
        message: {{ type: \"string\" }}
      }},
      required: [\"message\"]
    }},
    result: {{
      type: \"object\",
      properties: {{
        message: {{ type: \"string\" }}
      }},
      required: [\"message\"]
    }}
  }};
  async run(params) {{
    {}
    return Promise.resolve(main());
  }}
}};
// Annotate the CommonJS export names for ESM import in node:
0 && (module.exports = {{
  Tool
}});", js_code);

    // Create a minimal JSTool instance
    let tool = JSTool {
        toolkit_name: "deno".to_string(),
        name: "deno_runtime".to_string(),
        author: "system".to_string(),
        js_code: code,
        config: vec![],
        description: "Deno runtime execution".to_string(),
        keywords: vec![],
        input_args: vec![],
        activated: true,
        embedding: None,
        result: JSToolResult::new("object".to_string(), Value::Null, vec![]),
    };

    // Create a new parameters map without the code parameter
    let mut execution_parameters = parameters.clone();
    execution_parameters.remove("code");

    // Run the tool and convert the RunResult to Value
    match tool.run(execution_parameters, extra_config) {
        Ok(run_result) => Ok(run_result.data),
        Err(e) => Err(e),
    }
} 