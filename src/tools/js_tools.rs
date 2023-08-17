use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JSTool {
    pub toolkit_name: String,
    pub name: String,
    pub description: String,
    pub input_args: Vec<ToolArgument>,
    pub output_args: Vec<ToolArgument>,
}

impl JSTool {
    pub fn run(&self, _input_json: JsonValue) -> Result<(), ToolError> {
        // Implement the functionality here
        Ok(())
    }

    /// Returns a string that includes all of the input arguments' EBNF definitions,
    /// formatted such that the output specified is valid JSON structured as required
    /// to execute the JSTool with the external js tool executor.
    ///
    /// If `add_arg_descriptions` == true, then includes the arg descriptions as comments.
    pub fn ebnf_inputs(&self, add_arg_descriptions: bool) -> String {
        let mut ebnf_result = "{".to_string();
        let mut ebnf_arg_definitions = String::new();

        for input_arg in &self.input_args {
            let name = &input_arg.name;
            let ebnf = input_arg.labled_ebnf();

            ebnf_result.push_str(&format!(r#""{}": {}, "#, name, name));

            // Add descriptions to argument definitions if set to true
            if add_arg_descriptions {
                let description = &input_arg.description;
                let arg_ebnf = format!("{} (* {} *)\n", ebnf, description);
                ebnf_arg_definitions.push_str(&arg_ebnf);
            } else {
                let arg_ebnf = format!("{}\n", ebnf);
                ebnf_arg_definitions.push_str(&arg_ebnf);
            }
        }

        ebnf_result.push_str("}\n");
        ebnf_result.push_str(&ebnf_arg_definitions);
        ebnf_result
    }

    /// Parses a JSTool from a toolkit json
    pub fn from_toolkit_json(toolkit_name: &str, json: &JsonValue) -> Result<Self, ToolError> {
        let name = json["name"].as_str().ok_or(ToolError::ParseError("name".to_string()))?;
        let description = json["description"]
            .as_str()
            .ok_or(ToolError::ParseError("description".to_string()))?;

        let input_args_json = json["input"]
            .as_array()
            .ok_or(ToolError::ParseError("input".to_string()))?;
        let mut input_args = Vec::new();
        for arg in input_args_json {
            let tool_arg = ToolArgument::from_toolkit_json(arg)?;
            input_args.push(tool_arg);
        }

        let output_args_json = json["output"]
            .as_array()
            .ok_or(ToolError::ParseError("output".to_string()))?;
        let mut output_args = Vec::new();
        for arg in output_args_json {
            let tool_arg = ToolArgument::from_toolkit_json(arg)?;
            output_args.push(tool_arg);
        }

        Ok(Self {
            toolkit_name: toolkit_name.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            input_args,
            output_args,
        })
    }
}
