use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use crate::tools::js_tools::JSTool;
use crate::tools::rust_tools::{RustTool, RUST_TOOLKIT};
use serde_json;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::source::VRSourceReference;
use shinkai_vector_resources::vector_resource::{
    MapVectorResource, NodeContent, RetrievedNode, VectorResourceCore, VectorResourceSearch,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ShinkaiTool {
    Rust(RustTool),
    JS(JSTool),
}

impl ShinkaiTool {
    /// The key that this tool will be stored under in the tool router
    pub fn tool_router_key(&self) -> String {
        let (name, toolkit_name) = (
            self.name(),
            match self {
                ShinkaiTool::Rust(r) => r.toolkit_type_name(),
                ShinkaiTool::JS(j) => j.toolkit_name.to_string(),
            },
        );

        Self::gen_router_key(name, toolkit_name)
    }

    /// Tool name
    pub fn name(&self) -> String {
        match self {
            ShinkaiTool::Rust(r) => r.name.clone(),
            ShinkaiTool::JS(j) => j.name.clone(),
        }
    }
    /// Tool description
    pub fn description(&self) -> String {
        match self {
            ShinkaiTool::Rust(r) => r.description.clone(),
            ShinkaiTool::JS(j) => j.description.clone(),
        }
    }

    /// Toolkit name the tool is from
    pub fn toolkit_name(&self) -> String {
        match self {
            ShinkaiTool::Rust(r) => r.name.clone(),
            ShinkaiTool::JS(j) => j.name.clone(),
        }
    }

    /// Toolkit name the tool is from
    pub fn toolkit_type_name(&self) -> String {
        match self {
            ShinkaiTool::Rust(r) => r.toolkit_type_name().clone(),
            ShinkaiTool::JS(j) => j.toolkit_name.clone(),
        }
    }

    /// Returns the input arguments of the tool
    pub fn input_args(&self) -> Vec<ToolArgument> {
        match self {
            ShinkaiTool::Rust(r) => r.input_args.clone(),
            ShinkaiTool::JS(j) => j.input_args.clone(),
        }
    }

    /// Returns the output arguments of the tool
    pub fn output_args(&self) -> Vec<ToolArgument> {
        match self {
            ShinkaiTool::Rust(r) => r.output_args.clone(),
            ShinkaiTool::JS(j) => j.output_args.clone(),
        }
    }

    /// Returns a string that includes all of the input arguments' EBNF definitions
    pub fn ebnf_inputs(&self, add_arg_descriptions: bool) -> String {
        ToolArgument::generate_ebnf_for_args(
            self.input_args().clone(),
            self.toolkit_type_name().clone(),
            add_arg_descriptions,
        )
    }

    /// Returns a string that includes all of the input arguments' EBNF definitions
    pub fn ebnf_outputs(&self, add_arg_descriptions: bool) -> String {
        ToolArgument::generate_ebnf_for_args(
            self.output_args().clone(),
            self.toolkit_type_name().clone(),
            add_arg_descriptions,
        )
    }

    /// Returns a formatted summary of the tool
    pub fn formatted_tool_summary(&self, ebnf_output: bool) -> String {
        let mut summary = format!(
            "Tool Name: {}\nToolkit Name: {}\nDescription: {}\nTool Input EBNF: `{}`",
            self.name(),
            self.toolkit_type_name(),
            self.description(),
            self.ebnf_inputs(false)
        );

        if ebnf_output {
            summary.push_str(&format!("\nTool Output EBNF: `{}`", self.ebnf_outputs(false)));
        }

        summary
    }

    /// Returns a formatted summary of the tool
    pub fn xml_lite_formatted_tool_summary(&self, ebnf_output: bool) -> String {
        let mut summary = format!(
            "<toolkit name=\"{}\" description=\"{}\">",
            self.toolkit_name(),
            self.description(),
        );

        summary.push_str("<inputs>");
        summary.push_str(&ToolArgument::lite_xml_generate_ebnf_for_args(
            self.input_args().clone(),
            ebnf_output,
        ));
        summary.push_str("</inputs>");

        if ebnf_output {
            summary.push_str("<outputs>");
            summary.push_str(&ToolArgument::lite_xml_generate_ebnf_for_args(
                self.output_args().clone(),
                ebnf_output,
            ));
            summary.push_str("</outputs>");
        }

        summary.push_str("</toolkit>");

        summary
    }

    pub fn json_formatted_tool_summary(&self, ebnf_output: bool) -> Result<String, ToolError> {
        let mut summary = HashMap::new();

        summary.insert("toolkit_name", self.toolkit_name());
        summary.insert("description", self.description());

        let inputs = ToolArgument::json_generate_ebnf_for_args(self.input_args().clone(), ebnf_output);
        let inputs_json = serde_json::to_string(&inputs).map_err(|_| ToolError::FailedJSONParsing)?;
        summary.insert("inputs", inputs_json);

        if ebnf_output {
            let outputs = ToolArgument::json_generate_ebnf_for_args(self.output_args().clone(), ebnf_output);
            let outputs_json = serde_json::to_string(&outputs).map_err(|_| ToolError::FailedJSONParsing)?;
            summary.insert("outputs", outputs_json);
        }

        serde_json::to_string(&summary).map_err(|_| ToolError::FailedJSONParsing)
    }

    pub fn describe_formatted_tool_summary(&self, ebnf_output: bool) -> Result<String, ToolError> {
        let mut description = String::new();

        description.push_str(&format!("Toolkit Name: {}\n", self.toolkit_name()));
        description.push_str(&format!("Description: {}\n", self.description()));

        let inputs = ToolArgument::json_generate_ebnf_for_args(self.input_args().clone(), ebnf_output);
        for input in &inputs {
            description.push_str(&format!(
                "Input Name: {}\n",
                input.get("name").unwrap_or(&String::from("N/A"))
            ));
            if ebnf_output {
                description.push_str(&format!(
                    "Input EBNF: {}\n",
                    input.get("ebnf").unwrap_or(&String::from("N/A"))
                ));
            }
        }

        if ebnf_output {
            let outputs = ToolArgument::json_generate_ebnf_for_args(self.output_args().clone(), ebnf_output);
            for output in &outputs {
                description.push_str(&format!(
                    "Output Name: {}\n",
                    output.get("name").unwrap_or(&String::from("N/A"))
                ));
                description.push_str(&format!(
                    "Output EBNF: {}\n",
                    output.get("ebnf").unwrap_or(&String::from("N/A"))
                ));
            }
        }

        Ok(description)
    }

    pub fn csv_formatted_tool_summary(&self, ebnf_output: bool) -> String {
        let mut summary = String::from("Toolkit Name,Description,Inputs");

        if ebnf_output {
            summary.push_str(",Outputs");
        }

        summary.push('\n');

        summary.push_str(&format!(
            "{},{},{}",
            self.toolkit_name(),
            self.description(),
            ToolArgument::lite_csv_generate_ebnf_for_args(self.input_args().clone(), ebnf_output,),
        ));

        if ebnf_output {
            summary.push_str(&format!(
                ",{}",
                ToolArgument::lite_xml_generate_ebnf_for_args(self.output_args().clone(), ebnf_output,),
            ));
        }

        summary
    }

    /// Formats the tool's info into a String to be used for generating the tool's embedding.
    pub fn format_embedding_string(&self) -> String {
        let mut embedding_string = format!("{}:{}\n", self.name(), self.description());

        embedding_string.push_str("Input Args:\n");

        for arg in self.input_args() {
            embedding_string.push_str(&format!("-{}:{}\n", arg.name, arg.description));
        }

        embedding_string.push_str("Output Args:\n");

        for arg in self.output_args() {
            embedding_string.push_str(&format!("-{}:{}\n", arg.name, arg.description));
        }

        embedding_string
    }

    /// Generate the key that this tool will be stored under in the tool router
    pub fn gen_router_key(name: String, toolkit_name: String) -> String {
        // We replace any `/` in order to not have the names break VRPaths
        format!("{}:::{}", toolkit_name, name).replace('/', "|")
    }

    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Convert from json
    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json).map_err(|e| ToolError::ParseError(e.to_string()))?;
        Ok(deserialized)
    }
}

impl From<RustTool> for ShinkaiTool {
    fn from(tool: RustTool) -> Self {
        ShinkaiTool::Rust(tool)
    }
}

impl From<JSTool> for ShinkaiTool {
    fn from(tool: JSTool) -> Self {
        ShinkaiTool::JS(tool)
    }
}

/// A top level struct which indexes JSTools installed in the Shinkai Node
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolRouter {
    pub routing_resource: MapVectorResource,
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRouter {
    /// Create a new ToolRouter instance from scratch.
    pub fn new() -> Self {
        let name = "Tool Router";
        let desc = Some("Enables performing vector searches to find relevant tools.");
        let source = VRSourceReference::None;

        // Initialize the MapVectorResource and add all of the rust tools by default
        let mut routing_resource = MapVectorResource::new_empty(name, desc, source, true);
        let mut metadata = HashMap::new();
        metadata.insert(Self::tool_type_metadata_key(), Self::tool_type_rust_value());

        for t in RUST_TOOLKIT.rust_tool_map.values() {
            let tool = ShinkaiTool::Rust(t.clone());
            routing_resource.insert_text_node(
                tool.tool_router_key(),
                tool.to_json().unwrap(), // This unwrap should be safe because Rust Tools are not dynamic
                Some(metadata.clone()),
                t.tool_embedding.clone(),
                &vec![],
            );
        }

        ToolRouter {
            routing_resource,
        }
    }

    fn tool_type_metadata_key() -> String {
        "tool_type".to_string()
    }

    fn tool_type_rust_value() -> String {
        "rust".to_string()
    }

    fn tool_type_js_value() -> String {
        "js".to_string()
    }

    /// Fetches the ShinkaiTool from the ToolRouter by parsing the internal Node
    /// within the ToolRouter.
    pub fn get_shinkai_tool(&self, tool_name: &str, toolkit_name: &str) -> Result<ShinkaiTool, ToolError> {
        let key = ShinkaiTool::gen_router_key(tool_name.to_string(), toolkit_name.to_string());
        let node = self.routing_resource.get_root_node(key)?;
        ShinkaiTool::from_json(node.get_text_content()?)
    }

    /// A hard-coded DB key for the profile-wide Tool Router in Topic::Tools.
    /// No other resource is allowed to use this shinkai_db_key (this is enforced
    /// automatically because all resources have a two-part key)
    pub fn profile_router_shinkai_db_key() -> String {
        "profile_tool_router".to_string()
    }

    /// Returns a list of ShinkaiTools of the most similar that
    /// have matching data tag names.
    pub fn syntactic_vector_search(
        &self,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
    ) -> Vec<ShinkaiTool> {
        let nodes = self
            .routing_resource
            .syntactic_vector_search(query, num_of_results, data_tag_names);
        self.ret_nodes_to_tools(&nodes)
    }

    /// Returns a list of ShinkaiTools of the most similar.
    pub fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<ShinkaiTool> {
        let nodes = self.routing_resource.vector_search(query, num_of_results);
        self.ret_nodes_to_tools(&nodes)
    }

    /// Takes a list of RetrievedNodes and outputs a list of ShinkaiTools
    fn ret_nodes_to_tools(&self, ret_nodes: &Vec<RetrievedNode>) -> Vec<ShinkaiTool> {
        let mut shinkai_tools = vec![];
        for ret_node in ret_nodes {
            // Ignores tools added to the router which are invalid by matching on the Ok()
            if let Ok(data_string) = ret_node.node.get_text_content() {
                if let Ok(shinkai_tool) = ShinkaiTool::from_json(data_string) {
                    shinkai_tools.push(shinkai_tool);
                }
            }
        }
        shinkai_tools
    }

    /// Adds a tool into the ToolRouter instance.
    pub fn add_shinkai_tool(&mut self, shinkai_tool: &ShinkaiTool, embedding: Embedding) -> Result<(), ToolError> {
        let data = shinkai_tool.to_json()?;
        let router_key = shinkai_tool.tool_router_key();
        let metadata = None;

        // Setup the metadata based on tool type

        match self.routing_resource.get_root_node(router_key.clone()) {
            Ok(_) => {
                // If a Shinkai tool with same key is already found, error
                return Err(ToolError::ToolAlreadyInstalled(data.to_string()));
            }
            Err(_) => {
                // If no tool is found, insert new tool
                self.routing_resource._insert_kv_without_tag_validation(
                    &router_key,
                    NodeContent::Text(data),
                    metadata,
                    &embedding,
                    &vec![],
                );
            }
        }

        Ok(())
    }

    /// Deletes the tool inside of the ToolRouter given a valid id
    pub fn delete_shinkai_tool(&mut self, tool_name: &str, toolkit_name: &str) -> Result<(), ToolError> {
        let key = ShinkaiTool::gen_router_key(tool_name.to_string(), toolkit_name.to_string());
        self.routing_resource.print_all_nodes_exhaustive(None, false, false);
        println!("Tool key: {}", key);
        self.routing_resource.remove_node_dt_specified(key, None, true)?;
        Ok(())
    }

    /// Acquire the tool embedding for a given ShinkaiTool.
    pub fn get_tool_embedding(&self, shinkai_tool: &ShinkaiTool) -> Result<Embedding, ToolError> {
        Ok(self
            .routing_resource
            .get_root_embedding(shinkai_tool.tool_router_key().to_string())?)
    }

    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        Ok(ToolRouter {
            routing_resource: MapVectorResource::from_json(json)?,
        })
    }
    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }
}
