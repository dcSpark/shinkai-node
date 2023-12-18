use crate::base_vector_resources::BaseVectorResource;
use crate::base_vector_resources::VRBaseType;
use crate::data_tags::DataTag;
use crate::data_tags::DataTagIndex;
#[cfg(feature = "native-http")]
use crate::embedding_generator::EmbeddingGenerator;
#[cfg(feature = "native-http")]
use crate::embedding_generator::RemoteEmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::embeddings::MAX_EMBEDDING_STRING_SIZE;
use crate::metadata_index::MetadataIndex;
use crate::model_type::EmbeddingModelType;
use crate::resource_errors::VRError;
use crate::shinkai_time::ShinkaiTime;
use crate::source::VRLocation;
use crate::source::VRSource;
pub use crate::vector_resource_types::*;
pub use crate::vector_search_traversal::*;
use async_trait::async_trait;
use env_logger::filter::Filter;
use std::collections::HashMap;
use std::fs::Metadata;

/// Represents a VectorResource as an abstract trait that anyone can implement new variants of.
/// Of note, when working with multiple VectorResources, the `name` field can have duplicates,
/// but `resource_id` is expected to be unique.
#[async_trait]
pub trait VectorResource: Send + Sync {
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
    fn metadata_index(&self) -> &MetadataIndex;
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

    /// Updates the last_modified_datetime to the current time
    fn update_last_modified_to_now(&mut self) {
        let current_time = ShinkaiTime::generate_time_now();
        self.set_last_modified_datetime(current_time).unwrap();
    }

    #[cfg(feature = "native-http")]
    /// Initializes a `RemoteEmbeddingGenerator` that is compatible with this VectorResource
    /// (targets the same model and interface for embedding generation). Of note, you need
    /// to make sure the api_url/api_key match for the model used.
    fn initialize_compatible_embeddings_generator(
        &self,
        api_url: &str,
        api_key: Option<String>,
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
        let source_string = format!("Source: {}", self.source().format_source_string());

        // Take keywords until we hit an upper 500 character cap to ensure
        // we do not go past the embedding LLM context window.
        let pre_keyword_length = name.len() + desc.len() + source_string.len();
        let mut keyword_string = String::new();
        for phrase in keywords {
            if pre_keyword_length + keyword_string.len() + phrase.len() <= MAX_EMBEDDING_STRING_SIZE {
                keyword_string = format!("{}, {}", keyword_string, phrase);
            }
        }

        let mut result = format!("{}{}{}, Keywords: [{}]", name, source_string, desc, keyword_string);
        if result.len() > MAX_EMBEDDING_STRING_SIZE {
            result.truncate(MAX_EMBEDDING_STRING_SIZE);
        }
        result
    }

    /// Returns a "reference string" that uniquely identifies the VectorResource (formatted as: `{name}:::{resource_id}`).
    /// This is also used as the unique identifier of the Vector Resource in the VectorFS.
    fn reference_string(&self) -> String {
        VRHeader::generate_resource_reference_string(self.name().to_string(), self.resource_id().to_string())
    }

    /// Generates a VRHeader out of the VectorResource.
    /// Allows specifying a resource_location to identify where the VectorResource is
    /// being stored.
    fn generate_resource_header(&self, resource_location: Option<VRLocation>) -> VRHeader {
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
            self.created_datetime(),
            self.last_modified_datetime(),
            resource_location,
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
        let mut nodes = self.vector_search_customized(
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

                NodeContent::VRHeader(header) => {
                    println!("");
                    format!("Header For Vector Resource: {}", header.reference_string())
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

    #[cfg(feature = "native-http")]
    /// Performs a Dynamic Vector Search that returns the most similar nodes based on the query with
    /// default traversal method/options.
    /// Dynamic Vector Searches support internal VectorResources with different Embedding models by automatically generating
    /// the query Emedding from the input_query for each model. Dynamic Vector Searches are always Exhaustive.
    async fn dynamic_vector_search(
        &self,
        input_query: String,
        num_of_results: u64,
        embedding_generator: RemoteEmbeddingGenerator,
    ) -> Result<Vec<RetrievedNode>, VRError> {
        self.dynamic_vector_search_customized(
            input_query,
            num_of_results,
            &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
            None,
            embedding_generator,
        )
        .await
    }

    #[cfg(feature = "native-http")]
    /// Performs a dynamic vector search that returns the most similar nodes based on the input query String.
    /// The input options allows the developer to choose how the search interacts through the levels.
    /// Dynamic Vector Searches support internal VectorResources with different Embedding models by automatically generating
    /// the query Emedding from the input_query for each model. Dynamic Vector Searches are always Exhaustive.
    async fn dynamic_vector_search_customized(
        &self,
        input_query: String,
        num_of_results: u64,
        traversal_options: &Vec<TraversalOption>,
        starting_path: Option<VRPath>,
        embedding_generator: RemoteEmbeddingGenerator,
    ) -> Result<Vec<RetrievedNode>, VRError> {
        // We only traverse 1 level of depth at a time to be able to re-process the input_query as needed
        let mut traversal_options = traversal_options.clone();
        traversal_options.push(TraversalOption::UntilDepth(0));
        // Create a hash_map to save the embedding queries generated based on model type
        let mut input_query_embeddings: HashMap<EmbeddingModelType, Embedding> = HashMap::new();

        // First manually embedding generate & search the self Vector Resource
        let mut query_embedding = embedding_generator.generate_embedding_default(&input_query).await?;
        input_query_embeddings.insert(embedding_generator.model_type(), query_embedding.clone());
        let mut latest_returned_results = self.vector_search_customized(
            query_embedding,
            num_of_results,
            TraversalMethod::Exhaustive,
            &traversal_options,
            starting_path.clone(),
        );

        // Keep looping until we go through all nodes in the Vector Resource while carrying foward the score weighting
        // through the deeper levels of the Vector Resource
        let mut node_results = vec![];
        while let Some(ret_node) = latest_returned_results.pop() {
            match ret_node.node.content {
                NodeContent::Resource(ref resource) => {
                    // Reuse a previously generated query embedding if matching is available
                    if let Some(embedding) =
                        input_query_embeddings.get(&resource.as_trait_object().embedding_model_used())
                    {
                        query_embedding = embedding.clone();
                    }
                    // If a new embedding model is found for this resource, then initialize a new generator & generate the embedding
                    else {
                        let new_generator = resource.as_trait_object().initialize_compatible_embeddings_generator(
                            &embedding_generator.api_url,
                            embedding_generator.api_key.clone(),
                        );
                        query_embedding = new_generator.generate_embedding_default(&input_query).await?;
                        input_query_embeddings.insert(new_generator.model_type(), query_embedding.clone());
                    }

                    // Call vector_search() on the resource to get all the next depth Nodes from it
                    let new_results = resource.as_trait_object().vector_search_customized(
                        query_embedding,
                        num_of_results,
                        TraversalMethod::Exhaustive,
                        &traversal_options,
                        starting_path.clone(),
                    );
                    // Take into account current resource score, then push the new results to latest_returned_results to be further processed
                    if let Some(ScoringMode::HierarchicalAverageScoring) =
                        traversal_options.get_set_scoring_mode_option()
                    {
                        // Average resource's score into the retrieved results scores, then push them to latest_returned_results
                        for result in new_results {
                            let mut updated_result = result.clone();
                            updated_result.score =
                                vec![updated_result.score, ret_node.score].iter().sum::<f32>() / 2 as f32;
                            latest_returned_results.push(updated_result)
                        }
                    }
                }
                _ => {
                    // For any non-Vector Resource nodes, simply push them into the results
                    node_results.push(ret_node);
                }
            }
        }

        // Now that we have all of the node results, sort them efficiently and return the expected number of results
        let final_results = RetrievedNode::sort_by_score(&node_results, num_of_results);

        Ok(final_results)
    }

    /// Performs a vector search that returns the most similar nodes based on the query with
    /// default traversal method/options.
    fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<RetrievedNode> {
        self.vector_search_customized(
            query,
            num_of_results,
            TraversalMethod::Exhaustive,
            &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
            None,
        )
    }

    /// Performs a vector search that returns the most similar nodes based on the query.
    /// The input traversal_method/options allows the developer to choose how the search moves through the levels.
    /// The optional starting_path allows the developer to choose to start searching from a Vector Resource
    /// held internally at a specific path.
    fn vector_search_customized(
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
                        return resource.as_trait_object()._vector_search_customized_core(
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
        let mut results = self._vector_search_customized_core(
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
    fn _vector_search_customized_core(
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
        // Check for syntactic search prefilter mode
        let syntactic_search_option = traversal_options.iter().find_map(|option| match option {
            TraversalOption::SetPrefilterMode(PrefilterMode::SyntacticVectorSearch(data_tags)) => {
                Some(data_tags.clone())
            }
            _ => None,
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
        let mut query = query.clone();

        for (score, id) in scores {
            let mut skip_traversing_deeper = false;
            if let Ok(node) = self.get_node(id) {
                // Perform validations based on Filter Mode
                let filter_mode = traversal_options.get_set_filter_mode_option();
                if let Some(FilterMode::ContainsAnyMetadataKeyValues(kv_pairs)) = filter_mode.clone() {
                    if !FilterMode::node_metadata_any_check(&node, &kv_pairs) {
                        continue;
                    }
                }
                if let Some(FilterMode::ContainsAllMetadataKeyValues(kv_pairs)) = filter_mode {
                    if !FilterMode::node_metadata_all_check(&node, &kv_pairs) {
                        continue;
                    }
                }
                // Perform validations related to node content type
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
                                    resource_header: self.generate_resource_header(None),
                                    retrieval_path: traversal_path.clone(),
                                };
                                current_level_results.push(ret_node);
                                skip_traversing_deeper = true;
                                break;
                            }
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
                let sub_results = resource.as_trait_object()._vector_search_customized_core(
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
                        resource_header: self.generate_resource_header(None),
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
                    resource_header: self.generate_resource_header(None),
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
        self.vector_search_customized(
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
