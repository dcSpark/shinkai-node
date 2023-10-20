use crate::db::{ShinkaiDB, Topic};
use crate::resources::router::VectorResourceRouter;
use serde_json::from_str;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::base_vector_resources::{BaseVectorResource, VectorResourceBaseType};
use shinkai_vector_resources::document_resource::DocumentVectorResource;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::resource_errors::VectorResourceError;
use shinkai_vector_resources::vector_resource::{RetrievedNode, VectorResource, VectorResourcePointer};

use super::db::ProfileBoundWriteBatch;
use super::db_errors::*;

impl ShinkaiDB {
    /// Saves the supplied `VectorResourceRouter` into the ShinkaiDB as the profile resource router.
    fn save_profile_resource_router(
        &self,
        router: &VectorResourceRouter,
        profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let (bytes, cf) = self._prepare_profile_resource_router(router)?;

        // Insert into the "VectorResources" column family
        self.put_cf_pb(
            cf,
            &VectorResourceRouter::profile_router_shinkai_db_key(),
            bytes,
            profile,
        )?;

        Ok(())
    }

    /// Prepares the `VectorResourceRouter` for saving into the ShinkaiDB as the profile resource router.
    fn _prepare_profile_resource_router(
        &self,
        router: &VectorResourceRouter,
    ) -> Result<(Vec<u8>, &rocksdb::ColumnFamily), ShinkaiDBError> {
        // Convert JSON to bytes for storage
        let json = router.to_json()?;
        let bytes = json.as_bytes().to_vec(); // Clone the bytes here

        // Retrieve the handle for the "VectorResources" column family
        let cf = self.get_cf_handle(Topic::VectorResources)?;

        Ok((bytes, cf))
    }

    /// Saves the `VectorResource` into the ShinkaiDB in the resources topic as a JSON
    /// string.
    ///
    /// Note this is only to be used internally, as this does not add a resource
    /// pointer in the global VectorResourceRouter. Adding the pointer is required for any
    /// resource being saved and is implemented in `.save_resources`.
    fn _save_resource_pointerless(
        &self,
        resource: &BaseVectorResource,
        profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let (bytes, cf) = self._prepare_resource_pointerless(resource)?;

        // Insert into the "VectorResources" column family
        self.put_cf_pb(cf, &resource.as_trait_object().reference_string(), &bytes, profile)?;

        Ok(())
    }

    /// Prepares the `BaseVectorResource` for saving into the ShinkaiDB in the resources topic as a JSON
    /// string. Note this is only to be used internally.
    fn _prepare_resource_pointerless(
        &self,
        resource: &BaseVectorResource,
    ) -> Result<(Vec<u8>, &rocksdb::ColumnFamily), ShinkaiDBError> {
        // Convert BaseVectorResource JSON to bytes for storage
        let json = resource.to_json()?;
        let bytes = json.as_bytes().to_vec();

        // Retrieve the handle for the "VectorResources" column family
        let cf = self.get_cf_handle(Topic::VectorResources)?;

        Ok((bytes, cf))
    }

    /// Saves the `BaseVectorResource` into the ShinkaiDB. This updates the
    /// Global VectorResourceRouter with the resource pointers as well.
    ///
    /// Of note, if an existing resource exists in the DB with the same name and
    /// resource_id, this will overwrite the old resource completely.
    pub fn save_resource(&self, resource: BaseVectorResource, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        self.save_resources(vec![resource], profile)
    }

    /// Saves the list of `VectorResource`s into the ShinkaiDB. This updates the
    /// Profile VectorResourceRouter with the resource pointers as well.
    ///
    /// Of note, if an existing resource exists in the DB with the same name and
    /// resource_id, this will overwrite the old resource completely.
    pub fn save_resources(
        &self,
        resources: Vec<BaseVectorResource>,
        profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        // Get the resource router
        let mut router = self.get_profile_resource_router(profile)?;

        let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;
        for resource in resources {
            // Adds the JSON of the resource to the batch
            let (bytes, cf) = self._prepare_resource_pointerless(&resource)?;
            pb_batch.put_cf_pb(cf, &resource.as_trait_object().reference_string(), &bytes);

            // Add the pointer to the router, then putting the router
            // into the batch
            let pointer = resource.as_trait_object().get_resource_pointer();
            router.add_resource_pointer(&pointer)?;
            let (bytes, cf) = self._prepare_profile_resource_router(&router)?;
            pb_batch.put_cf_pb(cf, &VectorResourceRouter::profile_router_shinkai_db_key(), &bytes);
        }

        self.write_pb(pb_batch)?;

        Ok(())
    }

    /// Fetches the BaseVectorResource from the DB using a VectorResourcePointer
    pub fn get_resource_by_pointer(
        &self,
        resource_pointer: &VectorResourcePointer,
        profile: &ShinkaiName,
    ) -> Result<BaseVectorResource, ShinkaiDBError> {
        self.get_resource(&resource_pointer.reference.clone(), profile)
    }

    /// Fetches the BaseVectorResource from the DB
    pub fn get_resource(&self, key: &str, profile: &ShinkaiName) -> Result<BaseVectorResource, ShinkaiDBError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf_pb(Topic::VectorResources, key, profile)?;
        let json_str = std::str::from_utf8(&bytes)?;

        Ok(BaseVectorResource::from_json(json_str)?)
    }

    /// Fetches the Global VectorResource Router from  the DB
    pub fn get_profile_resource_router(&self, profile: &ShinkaiName) -> Result<VectorResourceRouter, ShinkaiDBError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf_pb(
            Topic::VectorResources,
            &VectorResourceRouter::profile_router_shinkai_db_key(),
            profile,
        )?;
        let json_str = std::str::from_utf8(&bytes)?;

        // Parse the JSON string into a DocumentVectorResource object
        let router: VectorResourceRouter = from_str(json_str)?;

        Ok(router)
    }

    /// Performs a 2-tier syntactic vector search across all resources.
    /// Only resources with matching data tags will be considered at all,
    /// and likewise only nodes with matching data tags inside of said
    /// resources will be scored and potentially returned.
    pub fn syntactic_vector_search(
        &self,
        query: Embedding,
        num_of_resources: u64,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
        profile: &ShinkaiName,
    ) -> Result<Vec<RetrievedNode>, ShinkaiDBError> {
        let resources =
            self.syntactic_vector_search_resources(query.clone(), num_of_resources, data_tag_names, profile)?;

        let mut retrieved_nodes = Vec::new();
        for resource in resources {
            println!("VectorResource: {}", resource.as_trait_object().name());
            let results =
                resource
                    .as_trait_object()
                    .syntactic_vector_search(query.clone(), num_of_results, data_tag_names);
            retrieved_nodes.extend(results);
        }

        Ok(RetrievedNode::sort_by_score(&retrieved_nodes, num_of_results))
    }

    /// Performs a 2-tier vector search across all resources using a query embedding.
    ///
    /// From there a vector search is performed on each resource with the query embedding,
    /// and the results from all resources are then collected, sorted, and the top num_of_results
    /// RetriedNodes based on similarity score are returned.
    pub fn vector_search(
        &self,
        query: Embedding,
        num_of_resources: u64,
        num_of_results: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<RetrievedNode>, ShinkaiDBError> {
        let resources = self.vector_search_resources(query.clone(), num_of_resources, profile)?;

        let mut retrieved_nodes = Vec::new();
        for resource in resources {
            let results = resource.as_trait_object().vector_search(query.clone(), num_of_results);
            retrieved_nodes.extend(results);
        }

        Ok(RetrievedNode::sort_by_score(&retrieved_nodes, num_of_results))
    }

    /// Performs a 2-tier vector search across all resources using a query embedding,
    /// returning retrieved nodes that are within a tolerance range of similarity.
    ///
    /// * `tolerance_range` - A float between 0 and 1, inclusive, that
    ///   determines the range of acceptable similarity scores as a percentage
    ///   of the highest score.
    pub fn vector_search_tolerance_ranged(
        &self,
        query: Embedding,
        num_of_resources: u64,
        tolerance_range: f32,
        profile: &ShinkaiName,
    ) -> Result<Vec<RetrievedNode>, ShinkaiDBError> {
        let retrieved_nodes = self.vector_search(query.clone(), num_of_resources, 1, profile)?;
        if retrieved_nodes.is_empty() {
            return Ok(Vec::new());
        }
        let top_node = &retrieved_nodes.get(0).ok_or(ShinkaiDBError::VectorResourceError(
            VectorResourceError::VectorResourceEmpty,
        ))?;

        // Fetch the nodes that fit in the tolerance range
        let resources = self.vector_search_resources(query.clone(), num_of_resources, profile)?;
        let mut final_nodes = Vec::new();
        for resource in resources {
            let results = resource.as_trait_object()._vector_search_tolerance_ranged_score(
                query.clone(),
                tolerance_range,
                top_node.score,
            );
            final_nodes.extend(results);
        }

        Ok(final_nodes)
    }

    /// Performs a 2-tier vector search using a query embedding across all DocumentVectorResources
    /// and fetches the most similar node + proximity_window number of nodes around it.
    ///
    /// Note: This only searches DocumentVectorResources in Topic::VectorResources, not all resources. This is
    /// because the proximity logic is not generic (potentially later we can have a Proximity trait).
    pub fn vector_search_proximity(
        &self,
        query: Embedding,
        num_of_docs: u64,
        proximity_window: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<RetrievedNode>, ShinkaiDBError> {
        let mut docs: Vec<DocumentVectorResource> = Vec::new();
        for doc in self.vector_search_docs(query.clone(), num_of_docs, profile)? {
            if let Ok(document_resource) = doc.as_document_resource() {
                docs.push(document_resource.clone());
            }
        }

        let mut retrieved_nodes = Vec::new();
        for doc in &docs {
            let results = doc.vector_search(query.clone(), 1);
            retrieved_nodes.extend(results);
        }

        let top_ret_nodes = RetrievedNode::sort_by_score(&retrieved_nodes, 1);
        let top_node = top_ret_nodes.get(0).ok_or(ShinkaiDBError::VectorResourceError(
            VectorResourceError::VectorResourceEmpty,
        ))?;

        for doc in &docs {
            if doc.reference_string() == top_node.resource_pointer.reference {
                return Ok(doc.vector_search_proximity(query, proximity_window)?);
            }
        }

        Err(ShinkaiDBError::VectorResourceError(
            VectorResourceError::VectorResourceEmpty,
        ))
    }

    /// Performs a syntactic vector search using a query embedding and list of data tag names.
    /// Returns num_of_resources amount of most similar VectorResources.
    pub fn syntactic_vector_search_resources(
        &self,
        query: Embedding,
        num_of_resources: u64,
        data_tag_names: &Vec<String>,
        profile: &ShinkaiName,
    ) -> Result<Vec<BaseVectorResource>, ShinkaiDBError> {
        let router = self.get_profile_resource_router(profile)?;
        let resource_pointers = router.syntactic_vector_search(query, num_of_resources, data_tag_names);

        let mut resources = vec![];
        for res_pointer in resource_pointers {
            resources.push(self.get_resource(&res_pointer.reference, profile)?);
        }

        Ok(resources)
    }

    /// Returns all resource pointers in the profile's Resource Router
    pub fn get_all_resource_pointers(
        &self,
        profile: &ShinkaiName,
    ) -> Result<Vec<VectorResourcePointer>, ShinkaiDBError> {
        let router = self.get_profile_resource_router(profile)?;
        Ok(router.get_all_resource_pointers())
    }

    /// Performs a vector search using a query embedding and returns the
    /// num_of_resources amount of most similar VectorResourcePointers.
    fn vector_search_resource_pointers(
        &self,
        query: Embedding,
        num_of_resources: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<VectorResourcePointer>, ShinkaiDBError> {
        let router = self.get_profile_resource_router(profile)?;
        Ok(router.vector_search(query, num_of_resources))
    }

    /// Performs a vector search using a query embedding and returns the
    /// num_of_resources amount of most similar BaseVectorResources.
    pub fn vector_search_resources(
        &self,
        query: Embedding,
        num_of_resources: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<BaseVectorResource>, ShinkaiDBError> {
        let resource_pointers = self.vector_search_resource_pointers(query, num_of_resources, profile)?;

        let mut resources = vec![];
        for res_pointer in resource_pointers {
            resources.push(self.get_resource(&res_pointer.reference, profile)?);
        }

        Ok(resources)
    }

    /// Performs a vector search using a query embedding and returns the
    /// num_of_docs amount of most similar DocumentVectorResources.
    pub fn vector_search_docs(
        &self,
        query: Embedding,
        num_of_docs: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<BaseVectorResource>, ShinkaiDBError> {
        let router = self.get_profile_resource_router(profile)?;
        let resource_pointers = router.vector_search(query, num_of_docs * 2);

        let mut resources = vec![];
        for res_pointer in resource_pointers {
            if res_pointer.resource_base_type == VectorResourceBaseType::Document {
                if (resources.len() as u64) < num_of_docs {
                    resources.push(self.get_resource(&res_pointer.reference, profile)?);
                }
            }
        }

        Ok(resources)
    }

    /// Creates a profile resource router if one does not exist in the DB.
    pub fn init_profile_resource_router(&self, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        if let Err(_) = self.get_profile_resource_router(profile) {
            let router = VectorResourceRouter::new();
            self.save_profile_resource_router(&router, profile)?;
        }
        Ok(())
    }
}
