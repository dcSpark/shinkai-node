use crate::db::{ShinkaiDB, Topic};
use crate::resources::document::DocumentResource;
use crate::resources::embedding_generator::RemoteEmbeddingGenerator;
use crate::resources::embeddings::Embedding;
use crate::resources::resource::RetrievedDataChunk;
use crate::resources::resource::{Resource, ResourceType};
use crate::resources::resource_errors::ResourceError;
use crate::resources::router::{ResourcePointer, ResourceRouter};
use rocksdb::{ColumnFamilyDescriptor, Error, IteratorMode, Options, DB};
use serde_json::{from_str, to_string};
use std::any::Any;

use super::db_errors::ShinkaiDBError;

impl ShinkaiDB {
    /// Saves the supplied `ResourceRouter` into the ShinkaiDB as the global router.
    fn save_global_resource_router(&self, router: &ResourceRouter) -> Result<(), ShinkaiDBError> {
        // Convert JSON to bytes for storage
        let json = router.to_json()?;
        let bytes = json.as_bytes();

        // Retrieve the handle for the "Resources" column family
        let cf = self.get_cf_handle(Topic::Resources)?;

        // Insert the message into the "Resources" column family
        self.db.put_cf(cf, ResourceRouter::global_router_db_key(), bytes)?;

        Ok(())
    }

    /// Saves the `Resource` into the ShinkaiDB in the resources topic as a JSON
    /// string.
    ///
    /// Note this is only to be used internally, as this does not add a resource
    /// pointer in the global ResourceRouter. Adding the pointer is required for any
    /// resource being saved and is implemented in `.save_resources`.
    fn save_resource_pointerless(&self, resource: &Box<dyn Resource>) -> Result<(), ShinkaiDBError> {
        // Convert Resource JSON to bytes for storage
        let json = resource.to_json()?;
        let bytes = json.as_bytes();

        // Retrieve the handle for the "Resources" column family
        let cf = self.get_cf_handle(Topic::Resources)?;

        // Insert the message into the "Resources" column family
        self.db.put_cf(cf, resource.db_key(), bytes)?;

        Ok(())
    }

    /// Saves the list of `Resource`s into the ShinkaiDB. This updates the
    /// Global ResourceRouter with the resource pointers as well.
    ///
    /// Of note, if an existing resource exists in the DB with the same name and
    /// resource_id, this will overwrite the old resource completely.
    pub fn save_resources(&self, resources: Vec<Box<dyn Resource>>) -> Result<(), ShinkaiDBError> {
        // Get the resource router
        let mut router = self.get_global_resource_router()?;

        // TODO: Batch saving the resource and the router together
        // to guarantee atomicity and coherence of router.
        for resource in resources {
            // Save the JSON of the resources in the DB
            self.save_resource_pointerless(&resource)?;
            // Add the pointer to the router, saving the router
            // to the DB on each iteration
            let pointer = ResourcePointer::from(&resource);
            router.add_resource_pointer(&pointer);
            self.save_global_resource_router(&router)?;
        }

        // Add logic here for dealing with the resource router

        Ok(())
    }

    /// Fetches the Resource from the DB
    pub fn get_resource<K: AsRef<[u8]>>(
        &self,
        key: K,
        resource_type: &ResourceType,
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
    pub fn get_document<K: AsRef<[u8]>>(&self, key: K) -> Result<DocumentResource, ShinkaiDBError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf(Topic::Resources, key)?;
        let json_str = std::str::from_utf8(&bytes)?;

        // Parse the JSON string into a Resource implementing struct
        Ok(from_str(json_str)?)
    }

    /// Fetches the Global Resource Router from  the DB
    pub fn get_global_resource_router(&self) -> Result<ResourceRouter, ShinkaiDBError> {
        // Fetch and convert the bytes to a valid UTF-8 string
        let bytes = self.get_cf(Topic::Resources, ResourceRouter::global_router_db_key())?;
        let json_str = std::str::from_utf8(&bytes)?;

        // Parse the JSON string into a DocumentResource object
        let router: ResourceRouter = from_str(json_str)?;

        Ok(router)
    }

    /// Performs a 2-tier vector similarity search across all resources using a query embedding.
    ///
    /// From there a similarity search is performed on each resource with the query embedding,
    /// and the results from all resources are then collected, sorted, and the top num_of_results
    /// RetriedDataChunks based on similarity score are returned.
    pub fn similarity_search_data(
        &self,
        query: Embedding,
        num_of_resources: u64,
        num_of_results: u64,
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let resources = self.similarity_search_resources(query.clone(), num_of_resources)?;

        let mut retrieved_chunks = Vec::new();
        for resource in resources {
            let results = resource.similarity_search(query.clone(), num_of_results);
            retrieved_chunks.extend(results);
        }

        // Sort retrieved_chunks in descending order of score.
        // TODO: In the future use a binary heap like in the resource
        // similarity_search(). Not as important here due to less chunks.
        retrieved_chunks.sort_unstable_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Only return the top num_of_results
        let num_of_results = num_of_results as usize;
        if retrieved_chunks.len() > num_of_results {
            retrieved_chunks.truncate(num_of_results);
        }

        Ok(retrieved_chunks)
    }

    /// Performs a 2-tier vector similarity search using a query embedding across all DocumentResources
    /// and fetches the most similar data chunk + proximity_window number of chunks around it.
    ///
    /// Note: This only searches DocumentResources in Topic::Resources, not all resources. This is
    /// because the proximity logic is not generic (potentially later we can have a Proximity trait).
    pub fn similarity_search_data_doc_proximity(
        &self,
        query: Embedding,
        num_of_docs: u64,
        proximity_window: u64,
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let docs = self.similarity_search_docs(query.clone(), num_of_docs)?;

        let mut retrieved_chunks = Vec::new();
        for doc in docs {
            let results = doc.similarity_search_proximity(query.clone(), proximity_window)?;
            retrieved_chunks.extend(results);
        }

        Ok(retrieved_chunks)
    }

    /// Performs a vector similarity search using a query embedding and returns the
    /// num_of_resources amount of most similar Resources.
    pub fn similarity_search_resources(
        &self,
        query: Embedding,
        num_of_resources: u64,
    ) -> Result<Vec<Box<dyn Resource>>, ShinkaiDBError> {
        let router = self.get_global_resource_router()?;
        let resource_pointers = router.similarity_search(query, num_of_resources);

        let mut resources = vec![];
        for res_pointer in resource_pointers {
            resources.push(self.get_resource(res_pointer.db_key, &(res_pointer.resource_type))?);
        }

        Ok(resources)
    }

    /// Performs a vector similarity search using a query embedding and returns the
    /// num_of_docs amount of most similar DocumentResources.
    pub fn similarity_search_docs(
        &self,
        query: Embedding,
        num_of_docs: u64,
    ) -> Result<Vec<DocumentResource>, ShinkaiDBError> {
        let router = self.get_global_resource_router()?;
        let resource_pointers = router.similarity_search(query, num_of_docs);

        let mut resources = vec![];
        for res_pointer in resource_pointers {
            resources.push(self.get_document(res_pointer.db_key)?);
        }

        Ok(resources)
    }

    /// Creates a global resource router if one does not exist in the DB.
    pub fn init_global_resource_router(&self) -> Result<(), ShinkaiDBError> {
        if let Err(_) = self.get_global_resource_router() {
            let router = ResourceRouter::new();
            self.save_global_resource_router(&router)?;
        }
        Ok(())
    }
}

mod tests {
    use super::*;
    use crate::resources::bert_cpp::BertCPPProcess;

    #[test]
    fn test_pdf_resource_save_to_db() {
        let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let generator = RemoteEmbeddingGenerator::new_default();

        // Read the pdf from file into a buffer
        let buffer = std::fs::read("files/shinkai_manifesto.pdf")
            .map_err(|_| ResourceError::FailedPDFParsing)
            .unwrap();

        // Generate DocumentResource
        let desc = "An initial manifesto of the Shinkai Network.";
        let doc = DocumentResource::parse_pdf(
            &buffer,
            100,
            &generator,
            "Shinkai Manifesto",
            Some(desc),
            Some("http://shinkai.com"),
        )
        .unwrap();

        // Init Database
        let db_path = format!("db_tests/{}", "embeddings");
        let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

        // Save/fetch doc
        let resource = Box::new(doc.clone()) as Box<dyn Resource>;
        shinkai_db.save_resource_pointerless(&resource).unwrap();
        let fetched_doc = shinkai_db.get_document(doc.db_key().clone()).unwrap();

        assert_eq!(doc, fetched_doc);
    }

    fn test_single_resource_similarity_search() {
        let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let generator = RemoteEmbeddingGenerator::new_default();

        // Read the pdf from file into a buffer
        let buffer = std::fs::read("files/shinkai_manifesto.pdf")
            .map_err(|_| ResourceError::FailedPDFParsing)
            .unwrap();

        // Generate DocumentResource
        let desc = "An initial manifesto of the Shinkai Network.";
        let doc = DocumentResource::parse_pdf(
            &buffer,
            100,
            &generator,
            "Shinkai Manifesto",
            Some(desc),
            Some("http://shinkai.com"),
        )
        .unwrap();

        // Init Database
        let db_path = format!("db_tests/{}", "embeddings");
        let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
        shinkai_db.init_global_resource_router();

        // Init a resource router

        let resource = Box::new(doc.clone()) as Box<dyn Resource>;
        shinkai_db.save_resources(vec![resource]).unwrap();
        // shinkai_db.save_resource_pointerless(&resource).unwrap();
        let fetched_doc = shinkai_db.get_document(doc.db_key().clone()).unwrap();

        assert_eq!(doc, fetched_doc);
    }
}
