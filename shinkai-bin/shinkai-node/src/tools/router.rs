use std::sync::{Arc, Weak};

use crate::db::ShinkaiDB;
use crate::tools::error::ToolError;
use crate::tools::rust_tools::RustTool;
use serde_json;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::source::VRSourceReference;
use shinkai_vector_resources::vector_resource::{
    MapVectorResource, NodeContent, RetrievedNode, VectorResourceCore, VectorResourceSearch,
};

use super::shinkai_tool::ShinkaiTool;

/// A top level struct which indexes Tools (Rust or JS or Workflows) installed in the Shinkai Node
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolRouter {
    pub routing_resource: MapVectorResource,
}

impl ToolRouter {
    /// Create a new ToolRouter instance from scratch.
    pub async fn new(generator: Box<dyn EmbeddingGenerator>, db: Weak<ShinkaiDB>, profile: ShinkaiName) -> Self {
        let name = "Tool Router";
        let desc = Some("Enables performing vector searches to find relevant tools.");
        let source = VRSourceReference::None;

        // Initialize the MapVectorResource and add all of the rust tools by default
        let mut routing_resource = MapVectorResource::new_empty(name, desc, source, true);

        // Add Rust tools
        Self::add_rust_tools(&mut routing_resource, generator.box_clone()).await;

        // Add JS tools
        if let Some(db) = db.upgrade() {
            Self::add_js_tools(&mut routing_resource, generator, db, profile).await;
        }

        ToolRouter { routing_resource }
    }

    async fn add_rust_tools(routing_resource: &mut MapVectorResource, generator: Box<dyn EmbeddingGenerator>) {
        // Generate the static Rust tools
        let rust_tools = RustTool::static_tools(generator).await;

        // print the total number of tools
        eprintln!("Total number of tools: {}", rust_tools.len());

        // Insert each Rust tool into the routing resource
        for tool in rust_tools {
            eprintln!("Inserting tool: {:?}", tool.name);
            eprintln!("Tool: {:?}", tool.clone());
            let shinkai_tool = ShinkaiTool::Rust(tool.clone());
            let _ = routing_resource.insert_text_node(
                shinkai_tool.tool_router_key(),
                shinkai_tool.to_json().unwrap(), // This unwrap should be safe because Rust Tools are not dynamic
                None,
                tool.tool_embedding.clone(),
                &vec![],
            );
        }
    }

    async fn add_js_tools(
        routing_resource: &mut MapVectorResource,
        generator: Box<dyn EmbeddingGenerator>,
        db: Arc<ShinkaiDB>,
        profile: ShinkaiName,
    ) {
        match db.all_tools_for_user(&profile) {
            Ok(tools) => {
                for tool in tools {
                    if let ShinkaiTool::JS(mut js_tool) = tool {
                        let js_lite_tool = js_tool.to_without_code();
                        let shinkai_tool = ShinkaiTool::JSLite(js_lite_tool);

                        let embedding = if let Some(embedding) = js_tool.embedding.clone() {
                            embedding
                        } else {
                            let new_embedding = generator
                                .generate_embedding_default(&shinkai_tool.format_embedding_string())
                                .await
                                .unwrap();
                            js_tool.embedding = Some(new_embedding.clone());
                            // Update the JS tool in the database
                            if let Err(e) = db.add_shinkai_tool(ShinkaiTool::JS(js_tool.clone()), profile.clone()) {
                                eprintln!("Error updating JS tool in DB: {:?}", e);
                            }
                            new_embedding
                        };

                        let _ = routing_resource.insert_text_node(
                            shinkai_tool.tool_router_key(),
                            shinkai_tool.to_json().unwrap(),
                            None,
                            embedding,
                            &vec![],
                        );
                    }
                }
            }
            Err(e) => eprintln!("Error fetching JS tools: {:?}", e),
        }
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
        self.routing_resource.remove_node_dt_specified(key, None, true)?;
        Ok(())
    }

    /// Fetches the ShinkaiTool from the ToolRouter by parsing the internal Node
    /// within the ToolRouter.
    pub fn get_shinkai_tool(&self, tool_name: &str, toolkit_name: &str) -> Result<ShinkaiTool, ToolError> {
        let key = ShinkaiTool::gen_router_key(tool_name.to_string(), toolkit_name.to_string());
        let node = self.routing_resource.get_root_node(key)?;
        ShinkaiTool::from_json(node.get_text_content()?)
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
