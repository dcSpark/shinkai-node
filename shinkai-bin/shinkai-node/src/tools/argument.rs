use std::collections::HashMap;

use crate::tools::error::ToolError;

use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolArgument {
    pub name: String,
    pub arg_type: String,
    pub description: String,
    pub is_optional: bool,
    pub wrapper_type: String,
    pub ebnf: String,
}

impl ToolArgument {
    /// Parses a ToolArgument from a toolkit json
    pub fn from_toolkit_json(json: &JsonValue) -> Result<Self, ToolError> {
        let name = json["name"].as_str().ok_or(ToolError::ParseError("name".to_string()))?;
        let arg_type = json["type"].as_str().ok_or(ToolError::ParseError("type".to_string()))?;
        let description = json["description"]
            .as_str()
            .ok_or(ToolError::ParseError("description".to_string()))?;
        let is_optional = json["isOptional"]
            .as_bool()
            .ok_or(ToolError::ParseError("isOptional".to_string()))?;
        let ebnf = json["ebnf"].as_str().ok_or(ToolError::ParseError("ebnf".to_string()))?;
        let wrapper_type = json["wrapperType"].as_str().unwrap_or("none");

        Ok(Self {
            name: name.to_string(),
            arg_type: arg_type.to_string(),
            description: description.to_string(),
            is_optional,
            wrapper_type: wrapper_type.to_string(),
            ebnf: ebnf.to_string(),
        })
    }

    /// Returns the ebnf definition with the name of the argument prepended
    /// properly in EBNF notation
    pub fn labled_ebnf(&self) -> String {
        format!("{} :== {}", self.name, self.ebnf)
    }

    /// Returns a string that includes all of the tool arguments' EBNF definitions,
    /// formatted such that the output specified is valid JSON structured as required
    /// to execute the tool.
    ///
    /// If `add_arg_descriptions` == true, then includes the arg descriptions as comments.
    pub fn generate_ebnf_for_args(args: Vec<ToolArgument>, toolkit_name: String, add_arg_descriptions: bool) -> String {
        let mut ebnf_result = "{".to_string();
        let mut ebnf_arg_definitions = String::new();

        for input_arg in args {
            let name = &input_arg.name;
            let ebnf = input_arg.labled_ebnf();

            // ebnf_result.push_str(&format!(r#""{}": {}, "#, name, name));
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

        // Add the toolkit name to the required inputs for the tool
        ebnf_result.push_str(&format!(r#""{}": {}, "#, "toolkit", toolkit_name));

        ebnf_result.push_str("}\n");
        ebnf_result.push_str(&ebnf_arg_definitions);
        ebnf_result
    }

    pub fn lite_csv_generate_ebnf_for_args(args: Vec<ToolArgument>, ebnf_output: bool) -> String {
        let mut csv_result = String::new();

        for input_arg in args {
            let name = &input_arg.name;
            let ebnf = input_arg.labled_ebnf();

            if ebnf_output {
                csv_result.push_str(&format!("{},{}", name, ebnf));
            } else {
                csv_result.push_str(name);
            }

            csv_result.push('\n');
        }

        csv_result
    }

    // Lite Representation
    pub fn lite_xml_generate_ebnf_for_args(args: Vec<ToolArgument>, add_arg_descriptions: bool) -> String {
        let mut xml_args = String::new();

        for arg in args {
            let arg_xml = if add_arg_descriptions {
                format!(
                    "<arg name=\"{}\" type=\"{}\" description=\"{}\"/>",
                    arg.name, arg.arg_type, arg.description
                )
            } else {
                format!("<arg name=\"{}\" type=\"{}\"/>", arg.name, arg.arg_type)
            };
            xml_args.push_str(&arg_xml);
        }

        xml_args
    }

    pub fn json_generate_ebnf_for_args(args: Vec<ToolArgument>, ebnf_output: bool) -> Vec<HashMap<String, String>> {
        let mut json_result = Vec::new();

        for input_arg in args {
            let mut arg_map = HashMap::new();
            arg_map.insert("name".to_string(), input_arg.name.clone());

            if ebnf_output {
                arg_map.insert("ebnf".to_string(), input_arg.labled_ebnf());
            }

            json_result.push(arg_map);
        }

        json_result
    }
}
