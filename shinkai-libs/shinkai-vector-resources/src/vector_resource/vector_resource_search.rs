use super::VectorResourceCore;
#[cfg(feature = "desktop-only")]
use crate::embedding_generator::EmbeddingGenerator;
#[cfg(feature = "desktop-only")]
use crate::embedding_generator::RemoteEmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::model_type::EmbeddingModelType;
use crate::resource_errors::VRError;
pub use crate::source::VRSourceReference;
pub use crate::vector_resource::vector_resource_types::*;
pub use crate::vector_resource::vector_search_traversal::*;
use async_trait::async_trait;
use rand::rngs::StdRng;
use rand::seq::IteratorRandom;
use rand::SeedableRng;
use std::collections::HashMap;

#[async_trait]
pub trait VectorResourceSearch: VectorResourceCore {
    #[cfg(feature = "desktop-only")]
    /// Fetches percent_to_verify (between 0.0 - 1.0) of random nodes from within the VectorResource
    /// and validates that said node's included embeddings in the VectorResource are correct.
    async fn verify_internal_embeddings_coherence(
        &self,
        generator: &dyn EmbeddingGenerator,
        percent_to_verify: f32,
    ) -> Result<bool, VRError> {
        let all_nodes = self.retrieve_nodes_exhaustive_unordered(None);
        let percent_to_verify = percent_to_verify.max(0.0).min(1.0);
        let num_to_verify = (all_nodes.len() as f32 * percent_to_verify).ceil() as usize;
        // Ensure at least one node is verified always
        let num_to_verify = num_to_verify.max(1);

        // Filter out any non-text nodes, and randomly select from these nodes for the list of nodes to be verified
        // TODO: Later on also allow VectorResource nodes, and re-generate the resource embedding + verify it.
        let mut rng = StdRng::from_entropy();
        let nodes_to_verify: Vec<_> = all_nodes
            .into_iter()
            .filter(|node| matches!(node.node.content, NodeContent::Text(_)))
            .choose_multiple(&mut rng, num_to_verify);

        for ret_node in nodes_to_verify {
            let embedding = self.retrieve_embedding_at_path(ret_node.retrieval_path)?;
            match ret_node.node.content {
                NodeContent::Text(text) => {
                    let regenerated_embedding = generator.generate_embedding_default(&text).await?;
                    // We check if the score of the regenerated embedding is ever below 0.99 (some leeway in case some models are not 100% deterministic)
                    let score = embedding.cosine_similarity(&regenerated_embedding) < 0.99;
                    if score {
                        return Ok(false);
                    }
                }
                _ => return Err(VRError::InvalidNodeType("Node must hold Text content".to_string())),
            }
        }

        Ok(true)
    }

    /// Returns every single node at any depth in the whole Vector Resource, including the Vector Resources nodes themselves,
    /// and the Nodes they hold additionally. If a starting_path is provided then fetches all nodes from there,
    /// else starts at root. If resources_only is true, only Vector Resources are returned.
    /// Of note: This method does not guarantee ordering of the nodes, no matter what kind of VR this is used on.
    fn retrieve_nodes_exhaustive_unordered(&self, starting_path: Option<VRPath>) -> Vec<RetrievedNode> {
        let empty_embedding = Embedding::new_empty();
        self.vector_search_customized(
            empty_embedding,
            0,
            TraversalMethod::UnscoredAllNodes,
            &vec![],
            starting_path,
        )
    }

    /// Retrieves any resource nodes from the Vector Resource at any level of depth under starting path.
    fn retrieve_resource_nodes_exhaustive(&self, starting_path: Option<VRPath>) -> Vec<RetrievedNode> {
        let mut nodes = self.retrieve_nodes_exhaustive_unordered(starting_path);
        nodes.retain(|node| matches!(node.node.content, NodeContent::Resource(_)));
        nodes
    }

    /// Retrieves any text nodes from the Vector Resource at any level of depth under starting path.
    fn retrieve_text_nodes_exhaustive(&self, starting_path: Option<VRPath>) -> Vec<RetrievedNode> {
        let mut nodes = self.retrieve_nodes_exhaustive_unordered(starting_path);
        nodes.retain(|node| matches!(node.node.content, NodeContent::Text(_)));
        nodes
    }

    /// Retrieves any external content nodes from the Vector Resource at any level of depth under starting path.
    fn retrieve_external_content_nodes_exhaustive(&self, starting_path: Option<VRPath>) -> Vec<RetrievedNode> {
        let mut nodes = self.retrieve_nodes_exhaustive_unordered(starting_path);
        nodes.retain(|node| matches!(node.node.content, NodeContent::ExternalContent(_)));
        nodes
    }

    /// Retrieves any VRHeader nodes from the Vector Resource at any level of depth under starting path.
    fn retrieve_vrheader_nodes_exhaustive(&self, starting_path: Option<VRPath>) -> Vec<RetrievedNode> {
        let mut nodes = self.retrieve_nodes_exhaustive_unordered(starting_path);
        nodes.retain(|node| matches!(node.node.content, NodeContent::VRHeader(_)));
        nodes
    }

    /// Retrieves all nodes and their paths to easily/quickly examine a Vector Resource.
    /// This is exhaustive and can begin from any starting_path.
    /// `shorten_data` - Cuts the string content short to improve readability.
    /// `resources_only` - Only prints Vector Resources
    /// `add_merkle_hash` - Adds the merkle hash to each node
    fn retrieve_all_nodes_contents_by_hierarchy(
        &self,
        starting_path: Option<VRPath>,
        shorten_data: bool,
        resources_only: bool,
        add_merkle_hash: bool,
    ) -> Vec<String> {
        let nodes = if resources_only {
            self.retrieve_resource_nodes_exhaustive(starting_path)
        } else {
            self.retrieve_nodes_exhaustive_unordered(starting_path)
        };

        let mut result = Vec::new();

        for node in nodes {
            let ret_path = node.retrieval_path;
            let _path = ret_path.format_to_string();
            let path_depth = ret_path.path_ids.len();
            let node_id = node.node.id.clone();
            let data = match &node.node.content {
                NodeContent::Text(s) => {
                    if shorten_data && s.chars().count() > 25 {
                        format!("{} - {}", node_id, s.chars().take(25).collect::<String>() + "...")
                    } else {
                        format!("{} - {}", node_id, s)
                    }
                }
                NodeContent::Resource(resource) => {
                    if path_depth == 1 {
                        eprintln!(" ");
                    }
                    // Decide what to print for start
                    format!(
                        "{} - {} <Folder> - {} Nodes Held Inside",
                        node_id,
                        resource.as_trait_object().name(),
                        resource.as_trait_object().get_root_embeddings().len()
                    )
                }
                NodeContent::ExternalContent(external_content) => {
                    format!("{} - {} <External Content>", node_id, external_content)
                }

                NodeContent::VRHeader(header) => {
                    format!("{} - {} <VRHeader>", node_id, header.reference_string())
                }
            };
            // Adding merkle hash if it exists to output string
            let mut merkle_hash = String::new();
            if add_merkle_hash {
                if let Ok(hash) = node.node.get_merkle_hash() {
                    if hash.chars().count() > 15 {
                        merkle_hash = hash.chars().take(15).collect::<String>() + "..."
                    } else {
                        merkle_hash = hash.to_string()
                    }
                }
            }

            // Create indent string and do the final print
            let indent_string = " ".repeat(path_depth * 2) + &">".repeat(path_depth);
            if merkle_hash.is_empty() {
                result.push(format!("{}{}", indent_string, data));
            } else {
                result.push(format!("{}{} | Merkle Hash: {}", indent_string, data, merkle_hash));
            }
        }

        result
    }

    /// Prints all nodes and their paths to easily/quickly examine a Vector Resource.
    /// This is exhaustive and can begin from any starting_path.
    /// `shorten_data` - Cuts the string content short to improve readability.
    /// `resources_only` - Only prints Vector Resources
    fn print_all_nodes_exhaustive(&self, starting_path: Option<VRPath>, shorten_data: bool, resources_only: bool) {
        let contents = self.retrieve_all_nodes_contents_by_hierarchy(starting_path, shorten_data, resources_only, true);

        for content in contents {
            eprintln!("{}", content);
        }
    }

    #[cfg(feature = "desktop-only")]
    /// Performs a dynamic vector search that returns the most similar nodes based on the input query String.
    /// Dynamic Vector Searches support internal VectorResources with different Embedding models by automatically generating
    /// the query Embedding from the input_query for each model. Dynamic Vector Searches are always Exhaustive.
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

    #[cfg(feature = "desktop-only")]
    /// Performs a dynamic vector search that returns the most similar nodes based on the input query String.
    /// Dynamic Vector Searches support internal VectorResources with different Embedding models by automatically generating
    /// the query Embedding from the input_query for each model. Dynamic Vector Searches are always Exhaustive.
    /// NOTE: Not all traversal_options (ex. UntilDepth) will work with Dynamic Vector Searches.
    async fn dynamic_vector_search_customized(
        &self,
        input_query: String,
        num_of_results: u64,
        traversal_options: &Vec<TraversalOption>,
        starting_path: Option<VRPath>,
        embedding_generator: RemoteEmbeddingGenerator,
    ) -> Result<Vec<RetrievedNode>, VRError> {
        // Setup the root VRHeader that will be attached to all RetrievedNodes
        let root_vr_header = self.generate_resource_header();
        // We only traverse 1 level of depth at a time to be able to re-process the input_query as needed
        let mut traversal_options = traversal_options.clone();
        traversal_options.retain(|option| !matches!(option, TraversalOption::UntilDepth(_)));
        traversal_options.push(TraversalOption::UntilDepth(0));
        // Create a hash_map to save the embedding queries generated based on model type
        let mut input_query_embeddings: HashMap<EmbeddingModelType, Embedding> = HashMap::new();

        // If the embedding model is different then initialize a new generator & generate the embedding
        let mut query_embedding = if self.embedding_model_used() != embedding_generator.model_type() {
            let new_generator = self.initialize_compatible_embeddings_generator(
                &embedding_generator.api_url,
                embedding_generator.api_key.clone(),
            );
            let query_embedding = new_generator.generate_embedding_default(&input_query).await?;
            input_query_embeddings.insert(new_generator.model_type(), query_embedding.clone());
            query_embedding
        } else {
            let query_embedding = embedding_generator.generate_embedding_default(&input_query).await?;
            input_query_embeddings.insert(embedding_generator.model_type(), query_embedding.clone());
            query_embedding
        };

        // Search the self Vector Resource
        let mut latest_returned_results = self.vector_search_customized(
            query_embedding,
            num_of_results,
            TraversalMethod::Exhaustive,
            &traversal_options,
            starting_path.clone(),
        );

        // Keep looping until we go through all nodes in the Vector Resource while carrying forward the score weighting
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
                    let new_results = resource.as_trait_object()._vector_search_customized_with_root_header(
                        query_embedding,
                        num_of_results,
                        TraversalMethod::Exhaustive,
                        &traversal_options,
                        starting_path.clone(),
                        Some(root_vr_header.clone()),
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
        // Call the new method, passing None for the root_header parameter
        let retrieved_nodes = self._vector_search_customized_with_root_header(
            query,
            num_of_results,
            traversal_method,
            traversal_options,
            starting_path,
            None,
        );

        if let VRSourceReference::Standard(SourceReference::FileRef(file_ref)) = self.source() {
            if let SourceFileType::Document(file_type) = file_ref.file_type {
                if file_type == DocumentFileType::Csv {
                    if let Some(first_node) = retrieved_nodes.first() {
                        if let Some(merged_node) = self.get_all_node_content_merged() {
                            let retrieved_node = RetrievedNode {
                                node: merged_node,
                                score: retrieved_nodes
                                    .iter()
                                    .reduce(|a, b| if a.score > b.score { a } else { b })
                                    .unwrap()
                                    .score,
                                resource_header: first_node.resource_header.clone(),
                                retrieval_path: first_node.retrieval_path.clone(),
                            };

                            return vec![retrieved_node];
                        }
                    }
                }
            }
        }

        retrieved_nodes
    }

    /// Vector search customized core logic, with ability to specify root_header
    fn _vector_search_customized_with_root_header(
        &self,
        query: Embedding,
        num_of_results: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
        starting_path: Option<VRPath>,
        root_header: Option<VRHeader>,
    ) -> Vec<RetrievedNode> {
        // Setup the root VRHeader that will be attached to all RetrievedNodes
        let root_vr_header = root_header.unwrap_or_else(|| self.generate_resource_header());

        // Only retrieve inner path if it exists and is not root
        if let Some(path) = starting_path {
            if path != VRPath::root() {
                match self.retrieve_node_at_path(path.clone(), None) {
                    Ok(ret_node) => {
                        if let NodeContent::Resource(resource) = ret_node.node.content.clone() {
                            return resource.as_trait_object()._vector_search_customized_core(
                                query,
                                num_of_results,
                                traversal_method,
                                traversal_options,
                                vec![],
                                path,
                                root_vr_header.clone(),
                            );
                        }
                    }
                    Err(_) => {}
                }
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
            root_vr_header.clone(),
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

        // Check if we need to adjust based on the ResultsMode
        if let Some(result_mode) = traversal_options.get_set_results_mode_option() {
            let ResultsMode::ProximitySearch(proximity_window, num_of_top_results) = result_mode;
            let mut paths_checked = HashMap::new();
            let mut new_results = Vec::new();
            let mut new_top_results_added = 0;
            let mut iter = results.iter().cloned();

            while new_top_results_added < num_of_top_results as usize {
                if let Some(top_result) = iter.next() {
                    // Check if the node has already been included, then skip
                    if paths_checked.contains_key(&top_result.retrieval_path.clone()) {
                        continue;
                    }

                    match self.proximity_retrieve_nodes_at_path(
                        top_result.retrieval_path.clone(),
                        proximity_window,
                        Some(query.clone()),
                    ) {
                        Ok(mut proximity_results) => {
                            let mut non_duplicates = vec![];
                            let top_result_path = top_result.retrieval_path.clone();
                            for proximity_result in &mut proximity_results {
                                // Replace the retrieved node with the actual top result node (to preserve results from other scoring logic, ie. hierarchical)
                                let mut proximity_result = if top_result_path == proximity_result.retrieval_path {
                                    top_result.clone()
                                } else {
                                    proximity_result.clone()
                                };

                                // Update the proximity result and push it into the list of non duplicate results
                                if !paths_checked.contains_key(&proximity_result.retrieval_path) {
                                    proximity_result.resource_header = root_vr_header.clone();
                                    proximity_result.set_proximity_group_id(new_top_results_added.to_string());
                                    paths_checked.insert(proximity_result.retrieval_path.clone(), true);
                                    non_duplicates.push(proximity_result.clone());
                                }
                            }

                            new_results.append(&mut non_duplicates);
                            new_top_results_added += 1;
                        }
                        Err(_) => new_results.push(top_result), // Keep the original result if proximity retrieval fails
                    }
                } else {
                    // Break the loop if there are no more top results to process
                    break;
                }
            }
            results = new_results;
        }

        // Check if we are using traversal method unscored all nodes
        if traversal_method != TraversalMethod::UnscoredAllNodes {
            results.truncate(num_of_results as usize);
        }

        results
    }

    /// Internal method which is used to keep track of traversal info
    #[allow(clippy::too_many_arguments)]
    fn _vector_search_customized_core(
        &self,
        query: Embedding,
        num_of_results: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
        hierarchical_scores: Vec<f32>,
        traversal_path: VRPath,
        root_vr_header: VRHeader,
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
                if let Ok(embedding) = self.get_root_embedding(id) {
                    embeddings_to_score.push(embedding);
                }
            }
        } else {
            // If SyntacticVectorSearch is not in traversal_options, get all embeddings
            embeddings_to_score = self.get_root_embeddings();
        }

        // Score embeddings based on traversal method
        let mut score_num_of_results = num_of_results;
        let scores = match traversal_method {
            // Score all if exhaustive
            TraversalMethod::Exhaustive => {
                score_num_of_results = embeddings_to_score.len() as u64;
                query.score_similarities(&embeddings_to_score, score_num_of_results)
            }
            // Fake score all as 0 if unscored exhaustive
            TraversalMethod::UnscoredAllNodes => embeddings_to_score
                .iter()
                .map(|embedding| (0.0, embedding.id.clone()))
                .collect(),
            // Else score as normal
            _ => query.score_similarities(&embeddings_to_score, score_num_of_results),
        };

        self._order_vector_search_results(
            scores,
            query,
            num_of_results,
            traversal_method,
            traversal_options,
            hierarchical_scores,
            traversal_path,
            root_vr_header,
        )
    }

    /// Internal method that orders all scores, and importantly traverses into any nodes holding further BaseVectorResources.
    #[allow(clippy::too_many_arguments)]
    fn _order_vector_search_results(
        &self,
        scores: Vec<(f32, String)>,
        query: Embedding,
        num_of_results: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
        hierarchical_scores: Vec<f32>,
        traversal_path: VRPath,
        root_vr_header: VRHeader,
    ) -> Vec<RetrievedNode> {
        let mut current_level_results: Vec<RetrievedNode> = vec![];
        let mut vector_resource_count = 0;
        let query = query.clone();

        for (score, id) in scores {
            let mut skip_traversing_deeper = false;
            if let Ok(node) = self.get_root_node(id.clone()) {
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

                    // If traversal option includes UntilDepth and we've reached the right level
                    // Don't recurse any deeper, just return current Node with BaseVectorResource
                    if let Some(d) = traversal_options.get_until_depth_option() {
                        if d == traversal_path.depth_inclusive() {
                            let ret_node = RetrievedNode {
                                node: node.clone(),
                                score,
                                resource_header: root_vr_header.clone(),
                                retrieval_path: traversal_path.clone(),
                            };
                            current_level_results.push(ret_node);
                            skip_traversing_deeper = true;
                        }
                    }

                    // If node Resource does not have same base type as LimitTraversalToType then
                    // then skip going deeper into it
                    if let Some(base_type) = traversal_options.get_limit_traversal_to_type_option() {
                        if &node_resource.resource_base_type() != base_type {
                            skip_traversing_deeper = true;
                        }
                    }

                    // If node does not pass the validation check then skip going deeper into it
                    if let Some((validation_func, hash_map)) =
                        traversal_options.get_limit_traversal_by_validation_with_map_option()
                    {
                        let node_path = traversal_path.push_cloned(id.clone());
                        if !validation_func(&node, &node_path, hash_map) {
                            skip_traversing_deeper = true;
                        }
                    }
                }
                if skip_traversing_deeper {
                    continue;
                }

                let results = self._recursive_data_extraction(
                    node.clone(),
                    score,
                    query.clone(),
                    num_of_results,
                    traversal_method.clone(),
                    traversal_options,
                    hierarchical_scores.clone(),
                    traversal_path.clone(),
                    root_vr_header.clone(),
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
    #[allow(clippy::too_many_arguments)]
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
        root_vr_header: VRHeader,
    ) -> Vec<RetrievedNode> {
        let mut current_level_results: Vec<RetrievedNode> = vec![];
        // Concat the current score into a new hierarchical scores Vec before moving forward
        let mut new_hierarchical_scores = [&hierarchical_scores[..], &[score]].concat();
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
                    root_vr_header.clone(),
                );

                // If traversing with UnscoredAllNodes, include the Vector Resource
                // nodes as well in the results, prepended before their nodes
                // held inside
                if traversal_method == TraversalMethod::UnscoredAllNodes {
                    current_level_results.push(RetrievedNode {
                        node: node.clone(),
                        score,
                        resource_header: root_vr_header.clone(),
                        retrieval_path: new_traversal_path,
                    });
                }

                current_level_results.extend(sub_results);
            }
            // If it's not a resource, it's a node which we need to return
            _ => {
                let mut score = score;
                for option in traversal_options {
                    if let TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring) = option {
                        // Perform score "averaging" here. We go with a simple additional approach rather than actual average, so that low/many hierarchy scores does not kill an actually valuable node
                        if let Some(current_score) = new_hierarchical_scores.pop() {
                            let hierarchical_count = new_hierarchical_scores.len();
                            let hierarchical_sum = new_hierarchical_scores.iter().sum::<f32>();
                            if hierarchical_count > 0 && hierarchical_sum > 0.0 {
                                let hierarchical_weight = 0.2;
                                let current_score_weight = 1.0 - hierarchical_weight;
                                let hierarchical_score =
                                    (hierarchical_sum / hierarchical_count as f32) * hierarchical_weight;
                                score = (current_score * current_score_weight) + hierarchical_score;
                            }
                        }
                        break;
                    }
                }
                current_level_results.push(RetrievedNode {
                    node: node.clone(),
                    score,
                    resource_header: root_vr_header.clone(),
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
        data_tag_names: &[String],
    ) -> Vec<RetrievedNode> {
        self.vector_search_customized(
            query,
            num_of_results,
            TraversalMethod::Exhaustive,
            &vec![
                TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring),
                TraversalOption::SetPrefilterMode(PrefilterMode::SyntacticVectorSearch(data_tag_names.to_owned())),
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

/// Function used by deep searches to "average" out the scores of the retrieved nodes
/// with the top level search score from the VRs themselves.
/// Uses the input strings for more advanced detection for how much to weigh the VR score vs the node score.
pub fn deep_search_scores_average_out(
    _query_text: Option<String>,
    vr_score: f32,
    _vr_description: String,
    node_score: f32,
    _node_content: String,
) -> f32 {
    // TODO: Later on do keyword extraction on query_text, and if the description or node content has any of the top 3, increase weighting accordingly
    // This might be too intensive to run rake on all results, so re-think this over later/test it.

    // Go with a simple additional approach rather than actual average, so that low vr_scores never decrease actual node scores
    let vr_weight = 0.2;
    let adjusted_vr_score = (vr_score * vr_weight).min(0.2);
    node_score + adjusted_vr_score // final score
}
