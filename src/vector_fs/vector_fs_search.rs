use super::vector_fs_types::FSItem;
use super::{vector_fs::VectorFS, vector_fs_error::VectorFSError, vector_fs_reader::VFSReader};
use crate::vector_fs::vector_fs_permissions::PermissionsIndex;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::source::SourceFileMap;
use shinkai_vector_resources::vector_resource::{
    BaseVectorResource, LimitTraversalMode, Node, NodeContent, ScoringMode, VRHeader,
};
use shinkai_vector_resources::{
    embeddings::Embedding,
    vector_resource::{
        RetrievedNode, TraversalMethod, TraversalOption, VRPath, VectorResource, VectorResourceCore,
        VectorResourceSearch,
    },
};
use std::collections::HashMap;

/// A retrieved node from within a Vector Resource inside of the VectorFS.
/// Includes the path of the FSItem in the VectorFS and the retrieved node
/// from the Vector Resource inside the FSItem's path.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FSRetrievedNode {
    fs_item_path: VRPath,
    pub resource_retrieved_node: RetrievedNode,
}

impl FSRetrievedNode {
    /// Creates a new FSRetrievedNode.
    pub fn new(fs_item_path: VRPath, resource_retrieved_node: RetrievedNode) -> Self {
        FSRetrievedNode {
            fs_item_path,
            resource_retrieved_node,
        }
    }
    /// Returns the path of the FSItem the node was from
    pub fn fs_item_path(&self) -> VRPath {
        self.fs_item_path.clone()
    }

    /// Returns the name of the FSItem the node was from
    pub fn fs_item_name(&self) -> String {
        self.resource_retrieved_node.resource_header.resource_name.to_string()
    }

    /// Returns the reference_string of the FSItem (db key where the VR is stored)
    pub fn reference_string(&self) -> String {
        self.resource_retrieved_node.resource_header.reference_string()
    }

    /// Returns the similarity score of the retrieved node
    pub fn score(&self) -> f32 {
        self.resource_retrieved_node.score
    }
}

impl VectorFS {
    /// Generates an Embedding for the input query to be used in a Vector Search in the VecFS.
    /// This automatically uses the correct default embedding model for the given profile.
    pub async fn generate_query_embedding(
        &self,
        input_query: String,
        profile: &ShinkaiName,
    ) -> Result<Embedding, VectorFSError> {
        let generator = self._get_embedding_generator(profile)?;
        Ok(generator.generate_embedding_default(&input_query).await?)
    }

    /// Generates an Embedding for the input query to be used in a Vector Search in the VecFS.
    /// This automatically uses the correct default embedding model for the given profile in reader.
    pub async fn generate_query_embedding_using_reader(
        &self,
        input_query: String,
        reader: &VFSReader,
    ) -> Result<Embedding, VectorFSError> {
        self.generate_query_embedding(input_query, &reader.profile).await
    }

    /// Performs a "deep" vector search into the VectorFS starting at the reader's path,
    /// first finding the num_of_resources_to_search_into most relevant FSItems, then performing another
    /// vector search into each Vector Resource (inside the FSItem) to find and return the highest scored nodes.
    pub async fn deep_vector_search(
        &mut self,
        reader: &VFSReader,
        query_text: String,
        num_of_resources_to_search_into: u64,
        num_of_results: u64,
    ) -> Result<Vec<FSRetrievedNode>, VectorFSError> {
        self.deep_vector_search_customized(
            reader,
            query_text,
            num_of_resources_to_search_into,
            num_of_results,
            vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
        )
        .await
    }

    /// Performs a "deep" vector search into the VectorFS starting at the reader's path,
    /// first finding the num_of_resources_to_search_into most relevant FSItems, then performing another
    /// vector search into each Vector Resource (inside the FSItem) to find and return the highest scored nodes.
    /// Allows specifying custom deep_traversal_options which are used when searching into the VRs themselves.
    pub async fn deep_vector_search_customized(
        &mut self,
        reader: &VFSReader,
        query_text: String,
        num_of_resources_to_search_into: u64,
        num_of_results: u64,
        deep_traversal_options: Vec<TraversalOption>,
    ) -> Result<Vec<FSRetrievedNode>, VectorFSError> {
        let query = self
            .generate_query_embedding_using_reader(query_text.clone(), &reader)
            .await?;

        let mut ret_nodes = Vec::new();
        let mut fs_path_hashmap = HashMap::new();
        let items = self.vector_search_fs_item(reader, query.clone(), num_of_resources_to_search_into)?;

        for item in items {
            if let Ok(new_reader) = reader.new_reader_copied_data(item.path.clone(), self) {
                if let Ok(resource) = self.retrieve_vector_resource(&new_reader) {
                    fs_path_hashmap.insert(resource.as_trait_object().reference_string(), item.path);

                    let generator = self._get_embedding_generator(&reader.profile)?;
                    let results = resource
                        .as_trait_object()
                        .dynamic_vector_search_customized(
                            query_text.clone(),
                            num_of_results,
                            &deep_traversal_options,
                            None,
                            generator,
                        )
                        .await?;
                    ret_nodes.extend(results);
                }
            }
        }

        let mut final_results = vec![];
        for node in RetrievedNode::sort_by_score(&ret_nodes, num_of_results) {
            let fs_path = fs_path_hashmap.get(&node.resource_header.reference_string()).ok_or(
                VectorFSError::FailedGettingFSPathOfRetrievedNode(node.resource_header.reference_string()),
            )?;
            final_results.push(FSRetrievedNode::new(fs_path.clone(), node))
        }
        Ok(final_results)
    }

    /// Performs a vector search into the VectorFS starting at the reader's path,
    /// returning the retrieved FSItems extracted from the VRHeader-holding nodes
    pub fn vector_search_fs_item(
        &self,
        reader: &VFSReader,
        query: Embedding,
        num_of_results: u64,
    ) -> Result<Vec<FSItem>, VectorFSError> {
        println!("Starting vector search fs_item----------------------");
        let ret_nodes =
            self._vector_search_core(reader, query, num_of_results, TraversalMethod::Exhaustive, &vec![])?;
        let internals = self.get_profile_fs_internals_read_only(&reader.profile)?;

        let mut fs_items = vec![];
        for ret_node in ret_nodes {
            if let NodeContent::VRHeader(_) = ret_node.node.content {
                println!(
                    "Merkle hash: {:?} -- Score: {}",
                    ret_node.node.get_merkle_hash(),
                    &ret_node.score
                );
                fs_items.push(FSItem::from_vr_header_node(
                    ret_node.node.clone(),
                    ret_node.retrieval_path,
                    &internals.last_read_index,
                )?)
            }
        }
        Ok(fs_items)
    }

    /// Performs a vector search into the VectorFS starting at the reader's path,
    /// returning the retrieved (BaseVectorResource, SourceFileMap) pairs of the most similar FSItems.
    /// Ignores FSItem results which the requester_name does not have permission to read.
    pub fn vector_search_vr_and_source_file_map(
        &mut self,
        reader: &VFSReader,
        query: Embedding,
        num_of_results: u64,
    ) -> Result<Vec<(BaseVectorResource, SourceFileMap)>, VectorFSError> {
        let items = self.vector_search_fs_item(reader, query, num_of_results)?;
        let mut results = vec![];

        // If all perms pass, push
        for item in items {
            if let Ok(new_reader) = reader.new_reader_copied_data(item.path.parent_path(), self) {
                if let Ok(res) = self.retrieve_vr_and_source_file_map_in_folder(&new_reader, item.name()) {
                    results.push(res);
                }
            }
        }
        Ok(results)
    }

    /// Performs a vector search into the VectorFS starting at the reader's path,
    /// returning the retrieved BaseVectorResources which are the most similar.
    /// Ignores FSItem (Vector Resource) results which the requester_name does not have permission to read.
    pub fn vector_search_vector_resource(
        &mut self,
        reader: &VFSReader,
        query: Embedding,
        num_of_results: u64,
    ) -> Result<Vec<BaseVectorResource>, VectorFSError> {
        let items = self.vector_search_fs_item(reader, query, num_of_results)?;
        let mut results = vec![];

        // If all perms pass, push
        for item in items {
            if let Ok(new_reader) = reader.new_reader_copied_data(item.path.parent_path(), self) {
                if let Ok(res) = self.retrieve_vector_resource_in_folder(&new_reader, item.name()) {
                    results.push(res);
                }
            }
        }
        Ok(results)
    }

    /// Performs a vector search into the VectorFS starting at the reader's path,
    /// returning the retrieved SourceFileMap which are the most similar.
    /// Ignores FSItem (SFM) results which the requester_name does not have permission to read.
    pub fn vector_search_source_file_map(
        &mut self,
        reader: &VFSReader,
        query: Embedding,
        num_of_results: u64,
    ) -> Result<Vec<SourceFileMap>, VectorFSError> {
        let items = self.vector_search_fs_item(reader, query, num_of_results)?;
        let mut results = vec![];

        // If all perms pass, push
        for item in items {
            if let Ok(new_reader) = reader.new_reader_copied_data(item.path.parent_path(), self) {
                if let Ok(res) = self.retrieve_source_file_map_in_folder(&new_reader, item.name()) {
                    results.push(res);
                }
            }
        }
        Ok(results)
    }

    /// Performs a vector search into the VectorFS starting at the reader's path,
    /// returning the retrieved VRHeaders extracted from the nodes
    pub fn vector_search_vr_header(
        &self,
        reader: &VFSReader,
        query: Embedding,
        num_of_results: u64,
    ) -> Result<Vec<VRHeader>, VectorFSError> {
        let ret_nodes =
            self._vector_search_core(reader, query, num_of_results, TraversalMethod::Exhaustive, &vec![])?;
        let mut vr_headers = Vec::new();

        for node in ret_nodes {
            if let NodeContent::VRHeader(vr_header) = node.node.content {
                vr_headers.push(vr_header);
            }
        }

        Ok(vr_headers)
    }

    /// Core method all VectorFS vector searches *must* use. Performs a vector search into the VectorFS at
    /// the specified path in reader, returning the retrieved VRHeader nodes.
    /// Automatically inspects traversal_options to guarantee folder permissions, and any other must-have options
    /// are always respected.
    fn _vector_search_core(
        &self,
        reader: &VFSReader,
        query: Embedding,
        num_of_results: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
    ) -> Result<Vec<RetrievedNode>, VectorFSError> {
        let mut traversal_options = traversal_options.clone();
        let internals = self.get_profile_fs_internals_read_only(&reader.profile)?;
        let stringified_permissions_map = internals
            .permissions_index
            .export_permissions_hashmap_with_reader(reader);

        // Search without unique scoring (ie. hierarchical) because "folders" have no content/real embedding.
        // Also remove any set traversal limit, so we can enforce folder permission traversal limiting.
        traversal_options.retain(|option| match option {
            TraversalOption::SetTraversalLimiting(_) | TraversalOption::SetScoringMode(_) => false,
            _ => true,
        });

        // Enforce folder permissions are respected
        traversal_options.push(TraversalOption::SetTraversalLimiting(
            LimitTraversalMode::LimitTraversalByValidationWithMap((
                _permissions_validation_func,
                stringified_permissions_map,
            )),
        ));

        let results = internals.fs_core_resource.vector_search_customized(
            query,
            num_of_results,
            traversal_method,
            &traversal_options,
            Some(reader.path.clone()),
        );

        Ok(results)
    }
}

/// Internal validation function used by all VectorFS vector searches, in order to validate permissions of
/// VR-holding nodes while the search is traversing.
fn _permissions_validation_func(_: &Node, path: &VRPath, hashmap: HashMap<VRPath, String>) -> bool {
    // If the specified path has no permissions, then the default is to now allow traversing deeper
    if !hashmap.contains_key(path) {
        return false;
    }

    // Fetch/parse the VFSReader from the hashmap
    let reader = match hashmap.get(&PermissionsIndex::vfs_reader_unique_path()) {
        Some(reader_json) => match VFSReader::from_json(reader_json) {
            Ok(reader) => reader,
            Err(_) => return false,
        },
        None => return false,
    };

    // Initialize the PermissionsIndex struct
    let perm_index = PermissionsIndex::from_hashmap(reader.profile.clone(), hashmap);

    perm_index
        .validate_read_permission(&reader.requester_name, path)
        .is_ok()
}
