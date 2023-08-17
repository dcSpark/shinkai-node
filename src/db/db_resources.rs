use crate::db::{ShinkaiDB, Topic};
use crate::resources::document::DocumentResource;
use crate::resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use crate::resources::embeddings::Embedding;
use crate::resources::resource::RetrievedDataChunk;
use crate::resources::resource::{Resource, ResourceType};
use crate::resources::resource_errors::ResourceError;
use crate::resources::router::{ResourcePointer, ResourceRouter};
use serde_json::{from_str, to_string};
use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;

use super::db_errors::*;

impl ShinkaiDB {
    /// Saves the supplied `ResourceRouter` into the ShinkaiDB as the global router.
    fn save_profile_resource_router(
        &self,
        router: &ResourceRouter,
        profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        // Convert JSON to bytes for storage
        let json = router.to_json()?;
        let bytes = json.as_bytes();

        // Retrieve the handle for the "Resources" column family
        let cf = self.get_cf_handle(Topic::Resources)?;

        // Insert the message into the "Resources" column family
        self.put_cf_pb(cf, &ResourceRouter::profile_router_db_key(), bytes, profile)?;

        Ok(())
    }

    /// Saves the `Resource` into the ShinkaiDB in the resources topic as a JSON
    /// string.
    ///
    /// Note this is only to be used internally, as this does not add a resource
    /// pointer in the global ResourceRouter. Adding the pointer is required for any
    /// resource being saved and is implemented in `.save_resources`.
    fn save_resource_pointerless(
        &self,
        resource: &Box<dyn Resource>,
        profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        // Convert Resource JSON to bytes for storage
        let json = resource.to_json()?;
        let bytes = json.as_bytes();

        // Retrieve the handle for the "Resources" column family
        let cf = self.get_cf_handle(Topic::Resources)?;

        // Insert the message into the "Resources" column family
        self.put_cf_pb(cf, &resource.db_key(), bytes, profile)?;

        Ok(())
    }

    /// Saves the `Resource` into the ShinkaiDB. This updates the
    /// Global ResourceRouter with the resource pointers as well.
    ///
    /// Of note, if an existing resource exists in the DB with the same name and
    /// resource_id, this will overwrite the old resource completely.
    pub fn save_resource(&self, resource: Box<dyn Resource>, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        self.save_resources(vec![resource], profile)
    }

    /// Saves the list of `Resource`s into the ShinkaiDB. This updates the
    /// Global ResourceRouter with the resource pointers as well.
    ///
    /// Of note, if an existing resource exists in the DB with the same name and
    /// resource_id, this will overwrite the old resource completely.
    pub fn save_resources(
        &self,
        resources: Vec<Box<dyn Resource>>,
        profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        // Get the resource router
        let mut router = self.get_profile_resource_router(profile)?;

        // TODO: Batch saving the resource and the router together
        // to guarantee atomicity and coherence of router.
        for resource in resources {
            println!("saving resource");
            // Save the JSON of the resources in the DB
            self.save_resource_pointerless(&resource, profile)?;
            // Add the pointer to the router, saving the router
            // to the DB on each iteration
            let pointer = resource.get_resource_pointer();
            router.add_resource_pointer(&pointer)?;
            self.save_profile_resource_router(&router, profile)?;
        }

        // Add logic here for dealing with the resource router

        Ok(())
    }

    /// Fetches the Resource from the DB using a ResourcePointer
    pub fn get_resource_by_pointer(
        &self,
        resource_pointer: &ResourcePointer,
        profile: &ShinkaiName,
    ) -> Result<Box<dyn Resource>, ShinkaiDBError> {
        self.get_resource(
            resource_pointer.db_key.clone(),
            &resource_pointer.resource_type,
            profile,
        )
    }

    /// Fetches the Resource from the DB
    pub fn get_resource<K: AsRef<[u8]>>(
        &self,
        key: K,
        resource_type: &ResourceType,
        profile: &ShinkaiName,
    ) -> Result<Box<dyn Resource>, ShinkaiDBError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf(Topic::Resources, key)?;
        let json_str = std::str::from_utf8(&bytes)?;

        // Parse the JSON string into a Resource implementing struct
        if resource_type == &ResourceType::Document {
            let document_resource: DocumentResource = from_str(json_str)?;
            Ok(Box::new(document_resource))
        } else {
            Err(ShinkaiDBError::from(ResourceError::InvalidResourceType))
        }
    }

    /// Fetches a DocumentResource from the DB
    pub fn get_document<K: AsRef<[u8]>>(
        &self,
        key: K,
        profile: &ShinkaiName,
    ) -> Result<DocumentResource, ShinkaiDBError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf(Topic::Resources, key)?;
        let json_str = std::str::from_utf8(&bytes)?;

        // Parse the JSON string into a Resource implementing struct
        Ok(from_str(json_str)?)
    }

    /// Fetches the Global Resource Router from  the DB
    pub fn get_profile_resource_router(&self, profile: &ShinkaiName) -> Result<ResourceRouter, ShinkaiDBError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf(Topic::Resources, ResourceRouter::profile_router_db_key())?;
        let json_str = std::str::from_utf8(&bytes)?;

        // Parse the JSON string into a DocumentResource object
        let router: ResourceRouter = from_str(json_str)?;

        Ok(router)
    }

    /// Performs a 2-tier syntactic vector search across all resources.
    /// Only resources with matching data tags will be considered at all,
    /// and likewise only data chunks with matching data tags inside of said
    /// resources will be scored and potentially returned.
    pub fn syntactic_vector_search_data(
        &self,
        query: Embedding,
        num_of_resources: u64,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
        profile: &ShinkaiName,
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let resources =
            self.syntactic_vector_search_resources(query.clone(), num_of_resources, data_tag_names, profile)?;

        let mut retrieved_chunks = Vec::new();
        for resource in resources {
            println!("Resource: {}", resource.name());
            let results = resource.syntactic_vector_search(query.clone(), num_of_results, data_tag_names);
            retrieved_chunks.extend(results);
        }

        // Sort retrieved_chunks in descending order of score.
        // TODO: In the future use a binary heap like in the resource
        // vector_search(). Not as important here due to less chunks.
        retrieved_chunks.sort_unstable_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Only return the top num_of_results
        let num_of_results = num_of_results as usize;
        if retrieved_chunks.len() > num_of_results {
            retrieved_chunks.truncate(num_of_results);
        }

        Ok(retrieved_chunks)
    }

    /// Performs a 2-tier vector search across all resources using a query embedding.
    ///
    /// From there a vector search is performed on each resource with the query embedding,
    /// and the results from all resources are then collected, sorted, and the top num_of_results
    /// RetriedDataChunks based on similarity score are returned.
    pub fn vector_search_data(
        &self,
        query: Embedding,
        num_of_resources: u64,
        num_of_results: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let resources = self.vector_search_resources(query.clone(), num_of_resources, profile)?;

        let mut retrieved_chunks = Vec::new();
        for resource in resources {
            let results = resource.vector_search(query.clone(), num_of_results);
            retrieved_chunks.extend(results);
        }

        // Sort retrieved_chunks in descending order of score.
        // TODO: In the future use a binary heap like in the resource
        // vector_search(). Not as important here due to less chunks.
        retrieved_chunks.sort_unstable_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Only return the top num_of_results
        let num_of_results = num_of_results as usize;
        if retrieved_chunks.len() > num_of_results {
            retrieved_chunks.truncate(num_of_results);
        }

        Ok(retrieved_chunks)
    }

    /// Performs a 2-tier vector search across all resources using a query embedding,
    /// returning retrieved data chunks that are within a tolerance range of similarity.
    ///
    /// * `tolerance_range` - A float between 0 and 1, inclusive, that
    ///   determines the range of acceptable similarity scores as a percentage
    ///   of the highest score.
    pub fn vector_search_data_tolerance_ranged(
        &self,
        query: Embedding,
        num_of_resources: u64,
        tolerance_range: f32,
        profile: &ShinkaiName,
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let retrieved_chunks = self.vector_search_data(query.clone(), num_of_resources, 1, profile)?;
        let top_chunk = &retrieved_chunks
            .get(0)
            .ok_or(ShinkaiDBError::ResourceError(ResourceError::ResourceEmpty))?;

        // Fetch the chunks that fit in the tolerance range
        let resources = self.vector_search_resources(query.clone(), num_of_resources, profile)?;
        let mut final_chunks = Vec::new();
        for resource in resources {
            let results =
                resource.vector_search_tolerance_ranged_score(query.clone(), tolerance_range, top_chunk.score);
            final_chunks.extend(results);
        }

        Ok(final_chunks)
    }

    /// Performs a 2-tier vector search using a query embedding across all DocumentResources
    /// and fetches the most similar data chunk + proximity_window number of chunks around it.
    ///
    /// Note: This only searches DocumentResources in Topic::Resources, not all resources. This is
    /// because the proximity logic is not generic (potentially later we can have a Proximity trait).
    pub fn vector_search_data_doc_proximity(
        &self,
        query: Embedding,
        num_of_docs: u64,
        proximity_window: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let docs = self.vector_search_docs(query.clone(), num_of_docs, profile)?;

        let mut retrieved_chunks = Vec::new();
        for doc in &docs {
            let results = doc.vector_search(query.clone(), 1);
            retrieved_chunks.extend(results);
        }

        // Sort retrieved_chunks in descending order of score.
        // TODO: In the future use a binary heap like in the resource
        // vector_search(). Not as important here due to less chunks.
        retrieved_chunks.sort_unstable_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        let top_chunk = retrieved_chunks
            .get(0)
            .ok_or(ShinkaiDBError::ResourceError(ResourceError::ResourceEmpty))?;

        for doc in &docs {
            if doc.db_key() == top_chunk.resource_pointer.db_key {
                return Ok(doc.vector_search_proximity(query, proximity_window)?);
            }
        }

        Err(ShinkaiDBError::ResourceError(ResourceError::ResourceEmpty))
    }

    /// Performs a syntactic vector search using a query embedding and list of data tag names.
    /// Returns num_of_resources amount of most similar Resources.
    pub fn syntactic_vector_search_resources(
        &self,
        query: Embedding,
        num_of_resources: u64,
        data_tag_names: &Vec<String>,
        profile: &ShinkaiName,
    ) -> Result<Vec<Box<dyn Resource>>, ShinkaiDBError> {
        let router = self.get_profile_resource_router(profile)?;
        let resource_pointers = router.syntactic_vector_search(query, num_of_resources, data_tag_names);

        let mut resources = vec![];
        for res_pointer in resource_pointers {
            resources.push(self.get_resource(res_pointer.db_key, &(res_pointer.resource_type), profile)?);
        }

        Ok(resources)
    }

    /// Performs a vector search using a query embedding and returns the
    /// num_of_resources amount of most similar Resources.
    pub fn vector_search_resources(
        &self,
        query: Embedding,
        num_of_resources: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<Box<dyn Resource>>, ShinkaiDBError> {
        let router = self.get_profile_resource_router(profile)?;
        let resource_pointers = router.vector_search(query, num_of_resources);

        let mut resources = vec![];
        for res_pointer in resource_pointers {
            resources.push(self.get_resource(res_pointer.db_key, &(res_pointer.resource_type), profile)?);
        }

        Ok(resources)
    }

    /// Performs a vector search using a query embedding and returns the
    /// num_of_docs amount of most similar DocumentResources.
    pub fn vector_search_docs(
        &self,
        query: Embedding,
        num_of_docs: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<DocumentResource>, ShinkaiDBError> {
        let router = self.get_profile_resource_router(profile)?;
        let resource_pointers = router.vector_search(query, num_of_docs);

        let mut resources = vec![];
        for res_pointer in resource_pointers {
            resources.push(self.get_document(res_pointer.db_key, profile)?);
        }

        Ok(resources)
    }

    /// Creates a global resource router if one does not exist in the DB.
    pub fn init_profile_resource_router(&self, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        if let Err(_) = self.get_profile_resource_router(profile) {
            let router = ResourceRouter::new();
            self.save_profile_resource_router(&router, profile)?;
        }
        Ok(())
    }
}
