use crate::base_vector_resources::BaseVectorResource;
use crate::base_vector_resources::VRBaseType;
use crate::data_tags::DataTag;
use crate::data_tags::DataTagIndex;
use crate::embedding_generator::EmbeddingGenerator;
#[cfg(feature = "native-http")]
use crate::embedding_generator::RemoteEmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::embeddings::MAX_EMBEDDING_STRING_SIZE;
use crate::model_type::EmbeddingModelType;
use crate::resource_errors::VRError;
use crate::source::VRSource;
pub use crate::vector_resource_types::*;
use async_trait::async_trait;

/// An enum that represents the different traversal approaches
/// supported by Vector Searching. In other words these allow the developer to
/// choose how the searching algorithm
#[derive(Debug, Clone, PartialEq)]
pub enum TraversalMethod {
    /// Efficiently only goes deeper into Vector Resources if they are the highest scored Nodes at their level.
    /// Will go infinitely deep until hitting a level where no BaseVectorResources are part of the highest scored.
    Efficient,
    /// Traverses through all levels of depth and scores all content holding nodes.
    Exhaustive,
    /// Iterates exhaustively going through all levels while doing absolutely no scoring/similarity checking,
    /// returning every single Node at any level. Also returns the Vector Resources in addition to their
    /// Nodes they hold inside, thus providing all nodes that exist within the root Vector Resource.
    /// Note: This is not for vector searching, but for retrieving all possible Nodes.
    UnscoredAllNodes,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TraversalOption {
    /// Limits traversal into deeper Vector Resources only if they match the provided VRBaseType
    LimitTraversalToType(VRBaseType),
    /// Limits returned result to be within a percentage range (0.0 - 1.0) of the highest scored result.
    /// For example, you can set a tolerance range of 0.1 which means only nodes with a similarity score
    /// within 10% of the top result will be returned.
    ToleranceRangeResults(f32),
    /// Limits returned results to be greater than a specific score (0.0 - 1.0)
    MinimumScore(f32),
    /// Efficiently traverses until (and including) the specified depth is hit (or until there are no more levels to go).
    /// Will return BaseVectorResource Nodes if they are the highest scored at the specified depth.
    /// Top/root level starts at 0, and so first level of depth into internal BaseVectorResources is thus 1.
    UntilDepth(u64),
    /// By default Vector Search scoring only weighs a node based on it's single embedding alone.
    /// Alternate scoring modes are available which allow weighing a node base on relative scores
    /// above/below/beside, or otherwise to get potentially higher quality results.
    SetScoringMode(ScoringMode),
    /// Set a prefilter mode while performing a vector search. These modes use pre-processed indices in the Vector Resource
    /// to efficiently filter out all unrelated nodes before performing any semantic search logic.
    SetPrefilterMode(PrefilterMode),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScoringMode {
    /// While traversing, averages out the score all the way to each final node. In other words, the final score
    /// of each node weighs-in the scores of the Vector Resources that it was inside all the way up to the root.
    HierarchicalAverageScoring,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrefilterMode {
    /// Perform a Syntactic Vector Search.
    /// A syntactic vector search efficiently pre-filters all Nodes held internally to a subset that
    /// matches the provided list of data tag names (Strings).
    SyntacticVectorSearch(Vec<String>),
}

/// Represents a VectorResource as an abstract trait that anyone can implement new variants of.
/// Of note, when working with multiple VectorResources, the `name` field can have duplicates,
/// but `resource_id` is expected to be unique.
#[async_trait]
pub trait VectorResource {
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn source(&self) -> VRSource;
    fn resource_id(&self) -> &str;
    fn resource_embedding(&self) -> &Embedding;
    fn set_resource_embedding(&mut self, embedding: Embedding);
    fn resource_base_type(&self) -> VRBaseType;
    fn embedding_model_used(&self) -> EmbeddingModelType;
    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType);
    fn data_tag_index(&self) -> &DataTagIndex;
    /// Retrieves an Embedding given its id, at the root level depth.
    fn get_embedding(&self, id: String) -> Result<Embedding, VRError>;
    /// Retrieves all Embeddings at the root level depth of the Vector Resource.
    fn get_embeddings(&self) -> Vec<Embedding>;
    /// Retrieves a Node given its id, at the root level depth.
    fn get_node(&self, id: String) -> Result<Node, VRError>;
    /// Retrieves all Nodes at the root level of the Vector Resource
    fn get_nodes(&self) -> Vec<Node>;
    /// ISO RFC3339 when then Vector Resource was created
    fn created_datetime(&self) -> String;
    /// ISO RFC3339 when then Vector Resource was last modified
    fn last_modified_datetime(&self) -> String;
    /// Set a RFC3339 Datetime of when then Vector Resource was last modified
    fn set_last_modified_datetime(&mut self, datetime: String) -> Result<(), VRError>;
    // Note we cannot add from_json in the trait due to trait object limitations
    fn to_json(&self) -> Result<String, VRError>;

    #[cfg(feature = "native-http")]
    /// Regenerates and updates the resource's embedding using the name/description/source
    /// and the provided keywords.
    async fn update_resource_embedding(
        &mut self,
        generator: &dyn EmbeddingGenerator,
        keywords: Vec<String>,
    ) -> Result<(), VRError> {
        let formatted = self.format_embedding_string(keywords);
        let new_embedding = generator.generate_embedding(&formatted, "RE").await?;
        self.set_resource_embedding(new_embedding);
        Ok(())
    }

    #[cfg(feature = "native-http")]
    /// Regenerates and updates the resource's embedding using the name/description/source
    /// and the provided keywords.
    fn update_resource_embedding_blocking(
        &mut self,
        generator: &dyn EmbeddingGenerator,
        keywords: Vec<String>,
    ) -> Result<(), VRError> {
        let formatted = self.format_embedding_string(keywords);
        let new_embedding = generator.generate_embedding_blocking(&formatted, "RE")?;
        self.set_resource_embedding(new_embedding);
        Ok(())
    }

    #[cfg(feature = "native-http")]
    /// Initializes a `RemoteEmbeddingGenerator` that is compatible with this VectorResource
    /// (targets the same model and interface for embedding generation). Of note, you need
    /// to make sure the api_url/api_key match for the model used.
    fn initialize_compatible_embeddings_generator(
        &self,
        api_url: &str,
        api_key: Option<&str>,
    ) -> Box<dyn EmbeddingGenerator> {
        Box::new(RemoteEmbeddingGenerator::new(
            self.embedding_model_used(),
            api_url,
            api_key,
        ))
    }

    /// Generates a formatted string that represents the text to be used for
    /// generating the resource embedding.
    fn format_embedding_string(&self, keywords: Vec<String>) -> String {
        let name = format!("Name: {}", self.name());
        let desc = self
            .description()
            .map(|description| format!(", Description: {}", description))
            .unwrap_or_default();
        let source_string = self.source().format_source_string();

        // Take keywords until we hit an upper 500 character cap to ensure
        // we do not go past the embedding LLM context window.
        let pre_keyword_length = name.len() + desc.len() + source_string.len();
        let mut keyword_string = String::new();
        for phrase in keywords {
            if pre_keyword_length + keyword_string.len() + phrase.len() <= MAX_EMBEDDING_STRING_SIZE {
                keyword_string = format!("{}, {}", keyword_string, phrase);
            }
        }

        format!("{}{}{}, Keywords: [{}]", name, desc, source_string, keyword_string)
    }

    /// Returns a "reference string" that uniquely identifies the VectorResource (formatted as: `{name}:::{resource_id}`).
    /// This is also used in the Shinkai Node as the key where the VectorResource is stored in the DB.
    fn reference_string(&self) -> String {
        VRHeader::generate_resource_reference_string(self.name().to_string(), self.resource_id().to_string())
    }

    /// Generates a VRHeader out of the VectorResource
    fn generate_resource_header(&self) -> VRHeader {
        // Fetch list of data tag names from the index
        let tag_names = self.data_tag_index().data_tag_names();
        let embedding = self.resource_embedding().clone();

        VRHeader::new(
            self.name(),
            self.resource_id(),
            self.resource_base_type(),
            Some(embedding),
            tag_names,
            self.source(),
        )
    }

    /// Validates whether the VectorResource has a valid BaseVectorResourceType by checking its .resource_base_type()
    fn is_base_vector_resource(&self) -> Result<(), VRError> {
        VRBaseType::is_base_vector_resource(self.resource_base_type())
    }

    /// Returns every single node at any level in the whole Vector Resource, including sub Vector Resources
    /// and the Nodes they hold. If a starting_path is provided then fetches all nodes from there,
    /// else starts at root. If resources_only is true, only Vector Resources are returned.
    fn get_nodes_exhaustive(&self, starting_path: Option<VRPath>, resources_only: bool) -> Vec<RetrievedNode> {
        let empty_embedding = Embedding::new("", vec![]);
        let mut nodes = self.vector_seach_customized(
            empty_embedding,
            0,
            TraversalMethod::UnscoredAllNodes,
            &vec![],
            starting_path,
        );

        if resources_only {
            nodes.retain(|node| matches!(node.node.content, NodeContent::Resource(_)));
        }

        nodes
    }

    /// Prints all nodes and their paths to easily/quickly examine a Vector Resource.
    /// This is exhaustive and can begin from any starting_path.
    /// `shorten_data` - Cuts the string content short to improve readability.
    /// `resources_only` - Only prints Vector Resources
    fn print_all_nodes_exhaustive(&self, starting_path: Option<VRPath>, shorten_data: bool, resources_only: bool) {
        let nodes = self.get_nodes_exhaustive(starting_path, resources_only);
        for node in nodes {
            let path = node.retrieval_path.format_to_string();
            let data = match &node.node.content {
                NodeContent::Text(s) => {
                    if shorten_data && s.chars().count() > 25 {
                        s.chars().take(25).collect::<String>() + "..."
                    } else {
                        s.to_string()
                    }
                }
                NodeContent::Resource(resource) => {
                    println!("");
                    format!(
                        "<{}> - {} Nodes Held Inside",
                        resource.as_trait_object().name(),
                        resource.as_trait_object().get_embeddings().len()
                    )
                }
                NodeContent::ExternalContent(external_content) => {
                    println!("");
                    format!("External: {}", external_content)
                }
            };
            println!("{}: {}", path, data);
        }
    }

    /// Retrieves a node, no matter its depth, given its path.
    /// If the path is invalid at any part, then method will error.
    fn get_node_with_path(&self, path: VRPath) -> Result<Node, VRError> {
        if path.path_ids.is_empty() {
            return Err(VRError::InvalidVRPath(path.clone()));
        }

        // Fetch the first node directly, then iterate through the rest
        let mut node = self.get_node(path.path_ids[0].clone())?;
        for id in path.path_ids.iter().skip(1) {
            match node.content {
                NodeContent::Resource(ref resource) => {
                    node = resource.as_trait_object().get_node(id.clone())?;
                }
                _ => {
                    if let Some(last) = path.path_ids.last() {
                        if id != last {
                            return Err(VRError::InvalidVRPath(path.clone()));
                        }
                    }
                }
            }
        }
        Ok(node)
    }

    /// Performs a vector search that returns the most similar nodes based on the query with
    /// default traversal method/options.
    fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<RetrievedNode> {
        self.vector_seach_customized(
            query,
            num_of_results,
            TraversalMethod::Exhaustive,
            &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
            None,
        )
    }

    /// Performs a vector search that returns the most similar nodes based on the query.
    /// The input trtaversal_method/options allows the developer to choose how the search moves through the levels.
    /// The optional starting_path allows the developer to choose to start searching from a Vector Resource
    /// held internally at a specific path.
    fn vector_seach_customized(
        &self,
        query: Embedding,
        num_of_results: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
        starting_path: Option<VRPath>,
    ) -> Vec<RetrievedNode> {
        if let Some(path) = starting_path {
            match self.get_node_with_path(path.clone()) {
                Ok(node) => {
                    if let NodeContent::Resource(resource) = node.content {
                        return resource.as_trait_object()._vector_seach_customized_core(
                            query,
                            num_of_results,
                            traversal_method,
                            traversal_options,
                            vec![],
                            path,
                        );
                    }
                }
                Err(_) => {}
            }
        }
        // Perform the vector search and continue forward
        let mut results = self._vector_seach_customized_core(
            query.clone(),
            num_of_results,
            traversal_method.clone(),
            traversal_options,
            vec![],
            VRPath::new(),
        );

        // After getting all results from the vector search, perform final filtering
        // Check if we need to cut results according to tolerance range
        let tolerance_range_option = traversal_options.iter().find_map(|option| {
            if let TraversalOption::ToleranceRangeResults(range) = option {
                Some(*range)
            } else {
                None
            }
        });
        if let Some(tolerance_range) = tolerance_range_option {
            results = self._tolerance_range_results(tolerance_range, &results);
        }

        // Check if we need to cut results according to the minimum score
        let min_score_option = traversal_options.iter().find_map(|option| {
            if let TraversalOption::MinimumScore(score) = option {
                Some(*score)
            } else {
                None
            }
        });
        if let Some(min_score) = min_score_option {
            results = results
                .into_iter()
                .filter(|ret_node| ret_node.score >= min_score)
                .collect();
        }

        // Check if we are using traveral method unscored all nodes
        if traversal_method != TraversalMethod::UnscoredAllNodes {
            results.truncate(num_of_results as usize);
        }

        results
    }

    /// Internal method which is used to keep track of traversal info
    fn _vector_seach_customized_core(
        &self,
        query: Embedding,
        num_of_results: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
        hierarchical_scores: Vec<f32>,
        traversal_path: VRPath,
    ) -> Vec<RetrievedNode> {
        // First we fetch the embeddings we want to score
        let mut embeddings_to_score = vec![];
        let syntactic_search_option = traversal_options.iter().find_map(|option| {
            if let TraversalOption::SetPrefilterMode(PrefilterMode::SyntacticVectorSearch(data_tags)) = option {
                Some(data_tags.clone())
            } else {
                None
            }
        });
        if let Some(data_tag_names) = syntactic_search_option {
            // If SyntacticVectorSearch is in traversal_options, fetch nodes with matching data tags
            let ids = self._syntactic_search_id_fetch(&data_tag_names);
            for id in ids {
                if let Ok(embedding) = self.get_embedding(id) {
                    embeddings_to_score.push(embedding);
                }
            }
        } else {
            // If SyntacticVectorSearch is not in traversal_options, get all embeddings
            embeddings_to_score = self.get_embeddings();
        }

        // Score embeddings based on traversal method
        let mut score_num_of_results = num_of_results;
        let mut scores = vec![];
        match traversal_method {
            // Score all if exhaustive
            TraversalMethod::Exhaustive => {
                score_num_of_results = embeddings_to_score.len() as u64;
                scores = query.score_similarities(&embeddings_to_score, score_num_of_results);
            }
            // Fake score all as 0 if unscored exhaustive
            TraversalMethod::UnscoredAllNodes => {
                scores = embeddings_to_score
                    .iter()
                    .map(|embedding| (0.0, embedding.id.clone()))
                    .collect();
            }
            // Else score as normal
            _ => {
                scores = query.score_similarities(&embeddings_to_score, score_num_of_results);
            }
        }

        self._order_vector_search_results(
            scores,
            query,
            num_of_results,
            traversal_method,
            traversal_options,
            hierarchical_scores,
            traversal_path,
        )
    }

    /// Internal method that orders all scores, and importantly traverses into any nodes holding further BaseVectorResources.
    fn _order_vector_search_results(
        &self,
        scores: Vec<(f32, String)>,
        query: Embedding,
        num_of_results: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
        hierarchical_scores: Vec<f32>,
        traversal_path: VRPath,
    ) -> Vec<RetrievedNode> {
        let mut current_level_results: Vec<RetrievedNode> = vec![];
        let mut vector_resource_count = 0;
        for (score, id) in scores {
            let mut skip_traversing_deeper = false;
            if let Ok(node) = self.get_node(id) {
                // Check if it's a resource
                if let NodeContent::Resource(node_resource) = node.content.clone() {
                    // Keep track for later sorting efficiency
                    vector_resource_count += 1;
                    for option in traversal_options {
                        // If traversal option includes UntilDepth and we've reached the right level
                        // Don't recurse any deeper, just return current Node with BaseVectorResource
                        if let TraversalOption::UntilDepth(d) = option {
                            if d == &traversal_path.depth_inclusive() {
                                let ret_node = RetrievedNode {
                                    node: node.clone(),
                                    score,
                                    resource_header: self.generate_resource_header(),
                                    retrieval_path: traversal_path.clone(),
                                };
                                current_level_results.push(ret_node);
                                skip_traversing_deeper = true;
                                break;
                            }
                            // If node Resource does not have same base type as LimitTraversalToType then
                            // then skip going deeper into it
                            if let TraversalOption::LimitTraversalToType(base_type) = option {
                                if &node_resource.resource_base_type() != base_type {
                                    skip_traversing_deeper = true;
                                }
                            }
                        }
                    }
                }
                if skip_traversing_deeper {
                    continue;
                }

                let results = self._recursive_data_extraction(
                    node,
                    score,
                    query.clone(),
                    num_of_results,
                    traversal_method.clone(),
                    traversal_options,
                    hierarchical_scores.clone(),
                    traversal_path.clone(),
                );
                current_level_results.extend(results);
            }
        }

        // If at least one vector resource exists in the Nodes then re-sort
        // after fetching deeper level results to ensure ordering is correct
        if vector_resource_count >= 1 && traversal_method != TraversalMethod::UnscoredAllNodes {
            return RetrievedNode::sort_by_score(&current_level_results, num_of_results);
        }
        // Otherwise just return 1st level results
        current_level_results
    }

    /// Internal method for recursing into deeper levels of Vector Resources
    fn _recursive_data_extraction(
        &self,
        node: Node,
        score: f32,
        query: Embedding,
        num_of_results: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
        hierarchical_scores: Vec<f32>,
        traversal_path: VRPath,
    ) -> Vec<RetrievedNode> {
        let mut current_level_results: Vec<RetrievedNode> = vec![];
        // Concat the current score into a new hierarchical scores Vec before moving forward
        let new_hierarchical_scores = [&hierarchical_scores[..], &[score]].concat();
        // Create a new traversal path with the node id
        let new_traversal_path = traversal_path.push_cloned(node.id.clone());

        match &node.content {
            NodeContent::Resource(resource) => {
                // If no data tag names provided, it means we are doing a normal vector search
                let sub_results = resource.as_trait_object()._vector_seach_customized_core(
                    query.clone(),
                    num_of_results,
                    traversal_method.clone(),
                    traversal_options,
                    new_hierarchical_scores,
                    new_traversal_path.clone(),
                );

                // If traversing with UnscoredAllNodes, include the Vector Resource
                // nodes as well in the results, prepended before their nodes
                // held inside
                if traversal_method == TraversalMethod::UnscoredAllNodes {
                    current_level_results.push(RetrievedNode {
                        node: node.clone(),
                        score,
                        resource_header: self.generate_resource_header(),
                        retrieval_path: new_traversal_path,
                    });
                }

                current_level_results.extend(sub_results);
            }
            _ => {
                let mut score = score;
                for option in traversal_options {
                    if let TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring) = option {
                        score = new_hierarchical_scores.iter().sum::<f32>() / new_hierarchical_scores.len() as f32;
                        break;
                    }
                }
                current_level_results.push(RetrievedNode {
                    node: node.clone(),
                    score,
                    resource_header: self.generate_resource_header(),
                    retrieval_path: new_traversal_path,
                });
            }
        }
        current_level_results
    }

    /// Ease-of-use function for performing a syntactic vector search. Uses exhaustive traversal and hierarchical average scoring.
    /// A syntactic vector search efficiently pre-filters all Nodes held internally to a subset that matches the provided list of data tag names.
    fn syntactic_vector_search(
        &self,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
    ) -> Vec<RetrievedNode> {
        self.vector_seach_customized(
            query,
            num_of_results,
            TraversalMethod::Exhaustive,
            &vec![
                TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring),
                TraversalOption::SetPrefilterMode(PrefilterMode::SyntacticVectorSearch(data_tag_names.clone())),
            ],
            None,
        )
    }

    /// Returns the most similar nodes within a specific range of the provided top similarity score.
    fn _tolerance_range_results(&self, tolerance_range: f32, results: &Vec<RetrievedNode>) -> Vec<RetrievedNode> {
        // Calculate the top similarity score
        let top_similarity_score = results.first().map_or(0.0, |ret_node| ret_node.score);

        // Clamp the tolerance_range to be between 0 and 1
        let tolerance_range = tolerance_range.max(0.0).min(1.0);

        // Calculate the range of acceptable similarity scores
        let lower_bound = top_similarity_score * (1.0 - tolerance_range);

        // Filter the results to only include those within the range of the top similarity score
        let mut filtered_results = Vec::new();
        for ret_node in results {
            if ret_node.score >= lower_bound && ret_node.score <= top_similarity_score {
                filtered_results.push(ret_node.clone());
            }
        }

        filtered_results
    }

    /// Fetches all nodes which contain tags matching the input name list
    /// (including fetching inside all depths of Vector Resources exhaustively)
    /// TODO: Fix the retrieval path/depth to be proper on retrieved nodes
    fn get_all_syntactic_matches(&self, data_tag_names: &Vec<String>) -> Vec<RetrievedNode> {
        // Fetch all nodes with matching data tags
        let mut matching_nodes = vec![];
        let ids = self._syntactic_search_id_fetch(data_tag_names);
        for id in ids {
            if let Ok(node) = self.get_node(id.clone()) {
                match node.content {
                    NodeContent::Resource(resource) => {
                        let sub_results = resource.as_trait_object().get_all_syntactic_matches(data_tag_names);
                        matching_nodes.extend(sub_results);
                    }
                    _ => {
                        let resource_header = self.generate_resource_header();
                        let retrieved_node = RetrievedNode {
                            node: node,
                            score: 0.0,
                            resource_header,
                            retrieval_path: VRPath::new(),
                        };
                        matching_nodes.push(retrieved_node);
                    }
                }
            }
        }

        matching_nodes
    }

    /// Internal method to fetch all node ids for syntactic searches
    fn _syntactic_search_id_fetch(&self, data_tag_names: &Vec<String>) -> Vec<String> {
        let mut ids = vec![];
        for name in data_tag_names {
            if let Some(node_ids) = self.data_tag_index().get_node_ids(&name) {
                ids.extend(node_ids.iter().map(|id| id.to_string()));
            }
        }
        ids
    }
}
