use crate::base_vector_resources::VectorResourceBaseType;
use crate::data_tags::DataTagIndex;
use crate::embedding_generator::EmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::embeddings::MAX_EMBEDDING_STRING_SIZE;
use crate::model_type::EmbeddingModelType;
use crate::resource_errors::VectorResourceError;
pub use crate::vector_resource_types::*;

/// An enum that represents the different traversal approaches
/// supported by Vector Searching. In other words these allow the developer to
/// choose how the searching algorithm decides to include/ignore DataChunks.
#[derive(Debug, Clone, PartialEq)]
pub enum TraversalMethod {
    /// Efficiently only goes deeper into Vector Resources if they are the highest scored DataChunks at their level.
    /// Will go infinitely deep until hitting a level where no BaseVectorResources are part of the highest scored.
    Efficient,
    /// Efficiently traverses until (and including) the specified depth is hit (or until there are no more levels to go).
    /// Will return BaseVectorResource DataChunks if they are the highest scored at the specified depth.
    /// Top level starts at 0, and so first level of depth into internal BaseVectorResources is thus 1.
    UntilDepth(u64),
    /// Does not skip over any DataChunks, traverses through all levels.
    Exhaustive,
}

/// Represents a VectorResource as an abstract trait that anyone can implement new variants of.
/// Of note, when working with multiple VectorResources, the `name` field can have duplicates,
/// but `resource_id` is expected to be unique.
pub trait VectorResource {
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn source(&self) -> Option<&str>;
    fn resource_id(&self) -> &str;
    fn resource_embedding(&self) -> &Embedding;
    fn set_resource_embedding(&mut self, embedding: Embedding);
    fn resource_base_type(&self) -> VectorResourceBaseType;
    fn embedding_model_used(&self) -> EmbeddingModelType;
    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType);
    fn chunk_embeddings(&self) -> Vec<Embedding>;
    fn data_tag_index(&self) -> &DataTagIndex;
    fn get_chunk_embedding(&self, id: String) -> Result<Embedding, VectorResourceError>;
    /// Retrieves a data chunk given its id.
    fn get_data_chunk(&self, id: String) -> Result<DataChunk, VectorResourceError>;
    // Note we cannot add from_json in the trait due to trait object limitations
    fn to_json(&self) -> Result<String, VectorResourceError>;

    /// Regenerates and updates the resource's embedding.
    fn update_resource_embedding(
        &mut self,
        generator: &dyn EmbeddingGenerator,
        keywords: Vec<String>,
    ) -> Result<(), VectorResourceError> {
        let formatted = self.format_embedding_string(keywords);
        let new_embedding = generator.generate_embedding(&formatted, "RE")?;
        self.set_resource_embedding(new_embedding);
        Ok(())
    }

    /// Generates a formatted string that represents the data to be used for the
    /// resource embedding.
    fn format_embedding_string(&self, keywords: Vec<String>) -> String {
        let name = format!("Name: {}", self.name());
        let desc = self
            .description()
            .map(|description| format!(", Description: {}", description))
            .unwrap_or_default();
        let source = self
            .source()
            .map(|source| format!(", Source: {}", source))
            .unwrap_or_default();

        // Take keywords until we hit an upper 500 character cap to ensure
        // we do not go past the embedding LLM context window.
        let pre_keyword_length = name.len() + desc.len() + source.len();
        let mut keyword_string = String::new();
        for phrase in keywords {
            if pre_keyword_length + keyword_string.len() + phrase.len() <= MAX_EMBEDDING_STRING_SIZE {
                keyword_string = format!("{}, {}", keyword_string, phrase);
            }
        }

        format!("{}{}{}, Keywords: [{}]", name, desc, source, keyword_string)
    }

    /// Returns a "reference string" which is formatted as: `{name}:{resource_id}`.
    /// This uniquely identifies the given VectorResource, and is used in VectorResourcePointer to
    /// make it easy to know what resource a RetrievedDataChunk is from (more informative than bare resource_id).
    ///
    /// This is also used in the Shinkai Node for the key where the VectorResource will be stored in the DB.
    fn reference_string(&self) -> String {
        let name = self.name().replace(" ", "_").replace(":", "_");
        let resource_id = self.resource_id().replace(" ", "_").replace(":", "_");
        format!("{}:{}", name, resource_id)
    }

    /// Generates a VectorResourcePointer out of the VectorResource
    fn get_resource_pointer(&self) -> VectorResourcePointer {
        // Fetch list of data tag names from the index
        let tag_names = self.data_tag_index().data_tag_names();
        let embedding = self.resource_embedding().clone();

        VectorResourcePointer::new(
            &self.reference_string(),
            self.resource_base_type(),
            Some(embedding),
            tag_names,
        )
    }

    /// Validates whether the VectorResource has a valid BaseVectorResourceType by checking its .resource_base_type()
    fn is_base_vector_resource(&self) -> Result<(), VectorResourceError> {
        VectorResourceBaseType::is_base_vector_resource(self.resource_base_type())
    }

    /// Performs a vector search that returns the most similar data chunks based on the query. Of note this uses `Efficient` traversal
    /// which goes over all Vector Resources held inside of self, and only searches inside of them if their resource embedding is sufficiently
    /// similar to meet the `num_of_results` at that given level of depth.
    fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<RetrievedDataChunk> {
        self.vector_search_with_traversal(query, num_of_results, &TraversalMethod::Efficient)
    }

    /// Performs a vector search that returns the most similar data chunks based on the query.
    /// The input TraversalMethod allows the developer to choose how the search moves through the levels.
    fn vector_search_with_traversal(
        &self,
        query: Embedding,
        num_of_results: u64,
        traversal: &TraversalMethod,
    ) -> Vec<RetrievedDataChunk> {
        self._vector_search_with_traversal_depth(query, num_of_results, traversal, 0)
    }

    /// Internal method which is used to keep track of depth of the traversal
    fn _vector_search_with_traversal_depth(
        &self,
        query: Embedding,
        num_of_results: u64,
        traversal: &TraversalMethod,
        depth: u64,
    ) -> Vec<RetrievedDataChunk> {
        // If exhaustive traversal, then return all
        let num_of_results = if traversal == &TraversalMethod::Exhaustive {
            (&self.chunk_embeddings()).len() as u64
        } else {
            num_of_results
        };

        // Fetch the ordered scores from the abstracted function
        let scores = query.score_similarities(&self.chunk_embeddings(), num_of_results);

        self._order_vector_search_results(scores, query, num_of_results, &vec![], &traversal, depth)
    }

    /// Performs a syntactic vector search, aka efficiently pre-filtering to only search through DataChunks matching the list of data tag names.
    /// Of note this uses `Efficient` traversal, which goes over all Vector Resources held inside of self, and only searches inside of them if they both have
    /// matching data tags and their resource embedding is sufficiently similar to meet the `num_of_results` at that given level of depth.
    fn syntactic_vector_search(
        &self,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
    ) -> Vec<RetrievedDataChunk> {
        self.syntactic_vector_search_with_traversal(query, num_of_results, data_tag_names, &TraversalMethod::Efficient)
    }

    /// Performs a syntactic vector search, aka efficiently pre-filtering to only search through DataChunks matching the list of data tag names.
    /// The input TraversalMethod allows the developer to choose how the search moves through the levels.
    fn syntactic_vector_search_with_traversal(
        &self,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
        traversal: &TraversalMethod,
    ) -> Vec<RetrievedDataChunk> {
        self._syntactic_vector_search_with_traversal_depth(query, num_of_results, data_tag_names, traversal, 0)
    }

    /// Internal method which is used to keep track of depth of the traversal
    fn _syntactic_vector_search_with_traversal_depth(
        &self,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
        traversal: &TraversalMethod,
        depth: u64,
    ) -> Vec<RetrievedDataChunk> {
        // Fetch all data chunks with matching data tags
        let mut matching_data_tag_embeddings = vec![];
        let ids = self._syntactic_search_id_fetch(data_tag_names);
        for id in ids {
            if let Ok(embedding) = self.get_chunk_embedding(id) {
                matching_data_tag_embeddings.push(embedding);
            }
        }

        // If exhaustive traversal, then return all
        let num_of_results = if traversal == &TraversalMethod::Exhaustive {
            matching_data_tag_embeddings.len() as u64
        } else {
            num_of_results
        };
        // Score the embeddings and return only num_of_results most similar
        let scores = query.score_similarities(&matching_data_tag_embeddings, num_of_results);

        self._order_vector_search_results(scores, query, num_of_results, data_tag_names, &traversal, depth)
    }

    /// (Not a Vector Search) Fetches all data chunks which contain tags matching the input name list
    /// (including fetching inside all levels of Vector Resources)
    fn syntactic_search(&self, data_tag_names: &Vec<String>) -> Vec<RetrievedDataChunk> {
        // Fetch all data chunks with matching data tags
        let mut matching_data_chunks = vec![];
        let ids = self._syntactic_search_id_fetch(data_tag_names);
        for id in ids {
            if let Ok(data_chunk) = self.get_data_chunk(id.clone()) {
                match data_chunk.data {
                    DataContent::Resource(resource) => {
                        let sub_results = resource.as_trait_object().syntactic_search(data_tag_names);
                        matching_data_chunks.extend(sub_results);
                    }
                    DataContent::Data(_) => {
                        let resource_pointer = self.get_resource_pointer();
                        let retrieved_data_chunk = RetrievedDataChunk {
                            chunk: data_chunk,
                            score: 0.0,
                            resource_pointer,
                            retrieval_depth: 0,
                        };
                        matching_data_chunks.push(retrieved_data_chunk);
                    }
                }
            }
        }

        matching_data_chunks
    }

    /// Internal method to fetch all chunk ids for syntactic searches
    fn _syntactic_search_id_fetch(&self, data_tag_names: &Vec<String>) -> Vec<String> {
        let mut ids = vec![];
        for name in data_tag_names {
            if let Some(chunk_ids) = self.data_tag_index().get_chunk_ids(&name) {
                ids.extend(chunk_ids.iter().map(|id| id.to_string()));
            }
        }
        ids
    }

    /// Internal method shared by vector_search() and syntactic_vector_search() that
    /// orders all scores, and importantly resolves any BaseVectorResources which were
    /// in the DataChunks of the most similar results.
    fn _order_vector_search_results(
        &self,
        scores: Vec<(f32, String)>,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
        traversal: &TraversalMethod,
        depth: u64,
    ) -> Vec<RetrievedDataChunk> {
        let mut current_level_results: Vec<RetrievedDataChunk> = vec![];
        let mut vector_resource_count = 0;
        for (score, id) in scores {
            if let Ok(chunk) = self.get_data_chunk(id) {
                // Check if it's a resource
                if let DataContent::Resource(_) = chunk.data {
                    // Keep track for later sorting efficiency
                    vector_resource_count += 1;

                    // If traversal method is UntilDepth and we've reached the right level
                    // Don't recurse any deeper, just return current DataChunk with BaseVectorResource
                    if let TraversalMethod::UntilDepth(d) = traversal {
                        if d == &depth {
                            let ret_chunk = RetrievedDataChunk {
                                chunk: chunk.clone(),
                                score,
                                resource_pointer: self.get_resource_pointer(),
                                retrieval_depth: depth,
                            };
                            current_level_results.push(ret_chunk);
                            continue;
                        }
                    }
                }

                let results = self._recursive_data_extraction(
                    chunk,
                    score,
                    query.clone(),
                    num_of_results,
                    data_tag_names,
                    traversal,
                    depth,
                );
                current_level_results.extend(results);
            }
        }

        // If at least one vector resource exists in the DataChunks then re-sort
        // after fetching deeper level results to ensure ordering are correct
        if vector_resource_count >= 1 {
            return RetrievedDataChunk::sort_by_score(&current_level_results, num_of_results);
        }
        // Otherwise just return 1st level results
        current_level_results
    }

    fn _recursive_data_extraction(
        &self,
        chunk: DataChunk,
        score: f32,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
        traversal: &TraversalMethod,
        depth: u64,
    ) -> Vec<RetrievedDataChunk> {
        let mut first_level_results: Vec<RetrievedDataChunk> = vec![];
        match chunk.data {
            DataContent::Resource(resource) => {
                // If no data tag names provided, it means we are doing a normal vector search
                let sub_results = if data_tag_names.is_empty() {
                    resource.as_trait_object()._vector_search_with_traversal_depth(
                        query.clone(),
                        num_of_results,
                        traversal,
                        depth + 1,
                    )
                } else {
                    resource
                        .as_trait_object()
                        ._syntactic_vector_search_with_traversal_depth(
                            query.clone(),
                            num_of_results,
                            data_tag_names,
                            traversal,
                            depth + 1,
                        )
                };
                first_level_results.extend(sub_results);
            }
            DataContent::Data(_) => {
                first_level_results.push(RetrievedDataChunk {
                    chunk: chunk.clone(),
                    score,
                    resource_pointer: self.get_resource_pointer(),
                    retrieval_depth: depth,
                });
            }
        }
        first_level_results
    }

    /// Performs a vector search using a query embedding and returns
    /// the most similar data chunks within a specific range.
    /// Automatically uses Efficient Traversal.
    ///
    /// * `tolerance_range` - A float between 0 and 1, inclusive, that
    ///   determines the range of acceptable similarity scores as a percentage
    ///   of the highest score.
    fn vector_search_tolerance_ranged(&self, query: Embedding, tolerance_range: f32) -> Vec<RetrievedDataChunk> {
        // Get top 100 results
        let results = self.vector_search(query.clone(), 100);

        // Calculate the top similarity score
        let top_similarity_score = results.first().map_or(0.0, |ret_chunk| ret_chunk.score);

        // Find the range of acceptable similarity scores
        self._vector_search_tolerance_ranged_score(query, tolerance_range, top_similarity_score)
    }

    /// Performs a vector search using a query embedding and returns
    /// the most similar data chunks within a specific range of the provided top similarity score.
    ///
    /// * `top_similarity_score` - A float that represents the top similarity score.
    fn _vector_search_tolerance_ranged_score(
        &self,
        query: Embedding,
        tolerance_range: f32,
        top_similarity_score: f32,
    ) -> Vec<RetrievedDataChunk> {
        // Clamp the tolerance_range to be between 0 and 1
        let tolerance_range = tolerance_range.max(0.0).min(1.0);

        let mut results = self.vector_search(query, 100);

        // Calculate the range of acceptable similarity scores
        let lower_bound = top_similarity_score * (1.0 - tolerance_range);

        // Filter the results to only include those within the range of the top similarity score
        results.retain(|ret_chunk| ret_chunk.score >= lower_bound && ret_chunk.score <= top_similarity_score);

        results
    }
}
