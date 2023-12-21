use serde_json;
use shinkai_vector_resources::base_vector_resources::VRBaseType;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::map_resource::MapVectorResource;
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::source::VRSource;
use shinkai_vector_resources::vector_resource::{NodeContent, RetrievedNode, VRHeader, VectorResource};
use shinkai_vector_resources::vector_resource_types::VRPath;
use std::collections::HashMap;
use std::str::FromStr;

/// A top level struct which indexes a series of VRHeaders using a MapVectorResource.
/// This is used in the DB to keep track of all saved Vector Resources.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorResourceRouter {
    routing_resource: MapVectorResource,
}

impl VectorResourceRouter {
    /// Create a new VectorResourceRouter instance from scratch.
    pub fn new() -> Self {
        let name = "VectorResource Router";
        let desc = Some("Enables performing vector searches to find relevant resources.");
        let source = VRSource::None;
        let resource_id = "resource_router";
        VectorResourceRouter {
            routing_resource: MapVectorResource::new_empty(name, desc, source, resource_id),
        }
    }

    /// A hard-coded DB key for the profile-wide Vector Resource Router in Topic::VectorResources.
    /// No other resource is allowed to use this shinkai_db_key (this is enforced
    /// automatically because all resources have a two-part key)
    pub fn profile_router_shinkai_db_key() -> String {
        "profile_resource_router".to_string()
    }

    /// Returns a list of VRHeaders of the most similar resources that have matching data tag names.
    /// Note: This uses the embedding attached to the VRHeader, which is expected to have been
    /// taken from the original Vector Resource.
    pub fn syntactic_vector_search(
        &self,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
    ) -> Vec<VRHeader> {
        let nodes = self
            .routing_resource
            .syntactic_vector_search(query, num_of_results, data_tag_names);
        self.ret_nodes_to_resource_headers(&nodes)
    }

    /// Returns a list of VRHeaders of the most similar resources.
    /// Note: This uses the embedding attached to the VRHeader, which is expected to have been
    /// taken from the original Vector Resource.
    pub fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<VRHeader> {
        let nodes = self.routing_resource.vector_search(query, num_of_results);
        self.ret_nodes_to_resource_headers(&nodes)
    }

    /// Returns all VRHeaders in the Resource Router
    pub fn get_all_resource_headers(&self) -> Vec<VRHeader> {
        let nodes = self.routing_resource.get_nodes();
        let map_resource_header = self.routing_resource.generate_resource_header(None);
        let mut resource_headers = vec![];

        for node in nodes {
            let retrieved_node = RetrievedNode {
                node: node.clone(),
                score: 0.0,
                resource_header: map_resource_header.clone(),
                retrieval_path: VRPath::new(),
            };

            let headers = self.ret_nodes_to_resource_headers(&vec![retrieved_node]);
            resource_headers.extend(headers);
        }

        resource_headers
    }

    /// Takes a list of RetrievedNodes and outputs a list of VRHeaders
    /// that point to the real resource (not the resource router).
    /// Of note, if a node holds an invalid VRBaseType or reference string then the node is ignored
    fn ret_nodes_to_resource_headers(&self, ret_nodes: &Vec<RetrievedNode>) -> Vec<VRHeader> {
        let mut resource_headers = vec![];
        for ret_node in ret_nodes {
            // Ignore resources added to the router with invalid resource types/reference_strings
            if let NodeContent::Text(data) = &ret_node.node.content {
                if let Ok(resource_base_type) = VRBaseType::from_str(data).map_err(|_| VRError::InvalidVRBaseType) {
                    let id = &ret_node.node.id;
                    let embedding = self.routing_resource.get_embedding(id.to_string()).ok();

                    // Extract the "source" field from the metadata
                    let source = ret_node
                        .node
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.get("source"))
                        .and_then(|source_json| VRSource::from_json(source_json).ok())
                        .unwrap_or(VRSource::None);

                    // Attempt to generate VRHeader using the reference string(shinkai db key).
                    let resource_header = VRHeader::new_with_reference_string(
                        id.to_string(),
                        resource_base_type,
                        embedding,
                        ret_node.node.data_tag_names.clone(),
                        source,
                        ret_node.node.last_modified_datetime.clone(),
                        ret_node.node.last_modified_datetime.clone(),
                        None,
                        vec![],
                    );
                    if let Ok(resource_header) = resource_header {
                        resource_headers.push(resource_header);
                    }
                }
            }
        }
        resource_headers
    }

    /// Extracts necessary data from a VRHeader to create a Node
    fn extract_resource_header_data(
        &self,
        resource_header: &VRHeader,
    ) -> Result<(String, String, Embedding, Option<HashMap<String, String>>), VRError> {
        let data = resource_header.resource_base_type.to_str().to_string();
        let embedding = resource_header
            .resource_embedding
            .clone()
            .ok_or(VRError::NoEmbeddingProvided)?;
        let shinkai_db_key = resource_header.reference_string();
        let metadata = match resource_header.resource_source.to_json() {
            Ok(source_json) => {
                let mut metadata_map = HashMap::new();
                metadata_map.insert("source".to_string(), source_json);
                Some(metadata_map)
            }
            Err(_) => None,
        };

        Ok((shinkai_db_key, data, embedding, metadata))
    }

    /// Adds a resource resource_header into the VectorResourceRouter instance.
    /// The resource_header is expected to have a valid resource embedding
    /// and the matching resource having already been saved into the DB.
    ///
    /// If a resource resource_header already exists with the same shinkai_db_key, then
    /// the old resource_header will be replaced.
    ///
    /// Of note, in this implementation we store the resource type in the `data`
    /// of the node and the shinkai db key (reference_string) as the id of the node.
    pub fn add_resource_header(&mut self, resource_header: &VRHeader) -> Result<(), VRError> {
        let (shinkai_db_key, data, embedding, metadata) = self.extract_resource_header_data(resource_header)?;
        let shinkai_db_key_clone = shinkai_db_key.clone();

        match self.routing_resource.get_node(shinkai_db_key_clone) {
            Ok(old_node) => {
                // If a resource resource_header with matching shinkai_db_key is found,
                // replace the existing resource resource_header with the new one.
                self.replace_resource_header(&old_node.id, resource_header)?;
            }
            Err(_) => {
                // If no resource resource_header with matching shinkai_db_key is found,
                // insert the new kv pair. We skip tag validation because the tags
                // have already been previously validated when adding into the
                // original resource.
                self.routing_resource._insert_kv_without_tag_validation(
                    &shinkai_db_key,
                    NodeContent::Text(data.to_string()),
                    metadata,
                    &embedding,
                    &resource_header.data_tag_names,
                );
            }
        }

        Ok(())
    }

    /// Replaces an existing resource resource_header with a new one
    pub fn replace_resource_header(
        &mut self,
        old_resource_header_id: &str,
        resource_header: &VRHeader,
    ) -> Result<(), VRError> {
        let (_, data, embedding, metadata) = self.extract_resource_header_data(resource_header)?;

        self.routing_resource._replace_kv_without_tag_validation(
            old_resource_header_id,
            NodeContent::Text(data.to_string()),
            metadata,
            &embedding,
            &resource_header.data_tag_names,
        )?;
        Ok(())
    }

    /// Deletes the resource resource_header inside of the VectorResourceRouter given a valid id
    pub fn delete_resource_header(&mut self, old_resource_header_id: &str) -> Result<(), VRError> {
        self.routing_resource.remove_node(old_resource_header_id)?;
        Ok(())
    }

    /// Acquire the resource_embedding for a given VRHeader.
    /// If the resource_header itself doesn't have the embedding attached to it,
    /// we use the id to fetch the embedding directly from the VectorResourceRouter.
    pub fn get_resource_embedding(&self, resource_header: &VRHeader) -> Result<Embedding, VRError> {
        if let Some(embedding) = resource_header.resource_embedding.clone() {
            Ok(embedding)
        } else {
            self.routing_resource.get_embedding(resource_header.reference_string())
        }
    }

    pub fn from_json(json: &str) -> Result<Self, VRError> {
        Ok(VectorResourceRouter {
            routing_resource: MapVectorResource::from_json(json)?,
        })
    }
    /// Convert to json
    pub fn to_json(&self) -> Result<String, VRError> {
        serde_json::to_string(self).map_err(|_| VRError::FailedJSONParsing)
    }
}
