use crate::db::{ShinkaiDB, Topic};
use crate::resources::document::DocumentResource;
use crate::resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use crate::resources::embeddings::Embedding;
use crate::resources::resource::RetrievedDataChunk;
use crate::resources::resource::{Resource, ResourceType};
use crate::resources::resource_errors::ResourceError;
use crate::resources::router::{ResourcePointer, ResourceRouter};
use rocksdb::{ColumnFamilyDescriptor, Error, IteratorMode, Options, DB};
use serde_json::{from_str, to_string};
use std::any::Any;
use std::fs;
use std::path::Path;

use super::db_errors::*;

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

    /// Saves the `Resource` into the ShinkaiDB. This updates the
    /// Global ResourceRouter with the resource pointers as well.
    ///
    /// Of note, if an existing resource exists in the DB with the same name and
    /// resource_id, this will overwrite the old resource completely.
    pub fn save_resource(&self, resource: Box<dyn Resource>) -> Result<(), ShinkaiDBError> {
        self.save_resources(vec![resource])
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
            println!("saving resource");
            // Save the JSON of the resources in the DB
            self.save_resource_pointerless(&resource)?;
            // Add the pointer to the router, saving the router
            // to the DB on each iteration
            let pointer = resource.get_resource_pointer();
            router.add_resource_pointer(&pointer)?;
            self.save_global_resource_router(&router)?;
        }

        // Add logic here for dealing with the resource router

        Ok(())
    }

    /// Fetches the Resource from the DB using a ResourcePointer
    pub fn get_resource_by_pointer(
        &self,
        resource_pointer: &ResourcePointer,
    ) -> Result<Box<dyn Resource>, ShinkaiDBError> {
        self.get_resource(resource_pointer.db_key.clone(), &resource_pointer.resource_type)
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
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let resources = self.vector_search_resources(query.clone(), num_of_resources)?;

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
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let retrieved_chunks = self.vector_search_data(query.clone(), num_of_resources, 1)?;
        let top_chunk = &retrieved_chunks
            .get(0)
            .ok_or(ShinkaiDBError::ResourceError(ResourceError::ResourceEmpty))?;

        println!("Top score: {}", top_chunk.score);

        // Fetch the chunks that fit in the tolerance range
        let resources = self.vector_search_resources(query.clone(), num_of_resources)?;
        let mut final_chunks = Vec::new();
        for resource in resources {
            println!("in resource");
            let results =
                resource.vector_search_tolerance_ranged_score(query.clone(), tolerance_range, top_chunk.score);
            println!("{:?}", results);
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
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let docs = self.vector_search_docs(query.clone(), num_of_docs)?;

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
        data_tag_names: Vec<String>,
    ) -> Result<Vec<Box<dyn Resource>>, ShinkaiDBError> {
        let router = self.get_global_resource_router()?;
        let resource_pointers = router.vector_search(query, num_of_resources);

        let mut resources = vec![];
        for res_pointer in resource_pointers {
            resources.push(self.get_resource(res_pointer.db_key, &(res_pointer.resource_type))?);
        }

        Ok(resources)
    }

    /// Performs a vector search using a query embedding and returns the
    /// num_of_resources amount of most similar Resources.
    pub fn vector_search_resources(
        &self,
        query: Embedding,
        num_of_resources: u64,
    ) -> Result<Vec<Box<dyn Resource>>, ShinkaiDBError> {
        let router = self.get_global_resource_router()?;
        let resource_pointers = router.vector_search(query, num_of_resources);

        let mut resources = vec![];
        for res_pointer in resource_pointers {
            resources.push(self.get_resource(res_pointer.db_key, &(res_pointer.resource_type))?);
        }

        Ok(resources)
    }

    /// Performs a vector search using a query embedding and returns the
    /// num_of_docs amount of most similar DocumentResources.
    pub fn vector_search_docs(
        &self,
        query: Embedding,
        num_of_docs: u64,
    ) -> Result<Vec<DocumentResource>, ShinkaiDBError> {
        let router = self.get_global_resource_router()?;
        let resource_pointers = router.vector_search(query, num_of_docs);

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

    fn setup() {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn test_pdf_resource_save_to_db() {
        setup();
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
            &vec![],
        )
        .unwrap();

        // Init Database
        let db_path = format!("db_tests/{}", "embeddings");
        let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
        shinkai_db.init_global_resource_router().unwrap();

        // Save/fetch doc
        let resource: Box<dyn Resource> = Box::new(doc.clone());
        shinkai_db.save_resource(resource).unwrap();
        let fetched_doc = shinkai_db.get_document(doc.db_key().clone()).unwrap();

        assert_eq!(doc, fetched_doc);
    }

    #[test]
    fn test_multi_resource_vector_search() {
        setup();
        let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let generator = RemoteEmbeddingGenerator::new_default();

        // Create a doc
        let mut doc = DocumentResource::new_empty(
            "3 Animal Facts",
            Some("A bunch of facts about animals and wildlife"),
            Some("animalwildlife.com"),
            "animal_resource",
        );

        doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice
        doc.update_resource_embedding(
            &generator,
            vec!["Dog".to_string(), "Camel".to_string(), "Seals".to_string()],
        )
        .unwrap();

        // Prepare embeddings + data, then add it to the doc
        let fact1 = "Dogs are creatures with 4 legs that bark.";
        let fact1_embeddings = generator.generate_embedding(fact1).unwrap();
        let fact2 = "Camels are slow animals with large humps.";
        let fact2_embeddings = generator.generate_embedding(fact2).unwrap();
        let fact3 = "Seals swim in the ocean.";
        let fact3_embeddings = generator.generate_embedding(fact3).unwrap();
        doc.append_data(fact1, None, &fact1_embeddings, &vec![]);
        doc.append_data(fact2, None, &fact2_embeddings, &vec![]);
        doc.append_data(fact3, None, &fact3_embeddings, &vec![]);

        // Read the pdf from file into a buffer
        let buffer = std::fs::read("files/shinkai_manifesto.pdf")
            .map_err(|_| ResourceError::FailedPDFParsing)
            .unwrap();

        // Generate DocumentResource
        let desc = "An initial manifesto of the Shinkai Network.";
        let doc2 = DocumentResource::parse_pdf(
            &buffer,
            100,
            &generator,
            "Shinkai Manifesto",
            Some(desc),
            Some("http://shinkai.com"),
            &vec![],
        )
        .unwrap();

        // Init Database
        let db_path = format!("db_tests/{}", "embeddings");
        let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
        shinkai_db.init_global_resource_router().unwrap();

        // Save resources to DB
        let resource1 = Box::new(doc.clone()) as Box<dyn Resource>;
        let resource2 = Box::new(doc2.clone()) as Box<dyn Resource>;
        shinkai_db.save_resources(vec![resource1, resource2]).unwrap();

        // Animal resource vector search
        let query = generator.generate_embedding("Animals").unwrap();
        let fetched_resources = shinkai_db.vector_search_resources(query, 100).unwrap();
        let fetched_doc = fetched_resources.get(0).unwrap();
        assert_eq!(&doc.resource_id(), &fetched_doc.resource_id());

        // Shinkai manifesto resource vector search
        let query = generator.generate_embedding("Shinkai").unwrap();
        let fetched_resources = shinkai_db.vector_search_resources(query, 1).unwrap();
        let fetched_doc = fetched_resources.get(0).unwrap();
        assert_eq!(&doc2.resource_id(), &fetched_doc.resource_id());

        // Camel DataChunk vector search
        let query = generator.generate_embedding("Camels").unwrap();
        let ret_data_chunks = shinkai_db.vector_search_data(query, 10, 10).unwrap();
        let ret_data_chunk = ret_data_chunks.get(0).unwrap();
        assert_eq!(fact2, &ret_data_chunk.chunk.data);

        // Camel DataChunk vector search
        let query = generator.generate_embedding("Does this relate to crypto?").unwrap();
        let ret_data_chunks = shinkai_db.vector_search_data(query, 10, 10).unwrap();
        let ret_data_chunk = ret_data_chunks.get(0).unwrap();
        assert_eq!(
            "With lessons derived from the P2P nature of blockchains, we in fact have all of the core primitives at hand to build a new AI coordinated computing paradigm that takes decentralization and user privacy seriously while offering native integration into the modern crypto stack.",
            &ret_data_chunk.chunk.data
        );

        // Camel DataChunk proximity vector search
        let query = generator.generate_embedding("Camel").unwrap();
        let ret_data_chunks = shinkai_db.vector_search_data_doc_proximity(query, 10, 2).unwrap();
        let ret_data_chunk = ret_data_chunks.get(0).unwrap();
        let ret_data_chunk2 = ret_data_chunks.get(1).unwrap();
        let ret_data_chunk3 = ret_data_chunks.get(2).unwrap();
        assert_eq!(fact1, &ret_data_chunk.chunk.data);
        assert_eq!(fact2, &ret_data_chunk2.chunk.data);
        assert_eq!(fact3, &ret_data_chunk3.chunk.data);

        // Animal tolerance range vector search
        let query = generator.generate_embedding("Animals that peform actions").unwrap();
        let ret_data_chunks = shinkai_db.vector_search_data_tolerance_ranged(query, 10, 0.4).unwrap();

        let ret_data_chunk = ret_data_chunks.get(0).unwrap();
        let ret_data_chunk2 = ret_data_chunks.get(1).unwrap();

        assert_eq!(fact1, &ret_data_chunk.chunk.data);
        assert_eq!(fact2, &ret_data_chunk2.chunk.data);

        // for ret_data in &ret_data_chunks {
        //         println!(
        //             "Origin: {}\nData: {}\nScore: {}\n\n",
        //             ret_data.resource_pointer.db_key, ret_data.chunk.data, ret_data.score
        //         )
        //     }
    }
}
