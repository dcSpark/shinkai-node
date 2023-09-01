// Assuming DataChunk, ResourcePointer, and VectorResource are defined somewhere else in your code
// use your_module::{DataChunk, ResourcePointer, VectorResource};

use crate::resources::{
    router::VectorResourcePointer,
    vector_resource::{DataChunk, RetrievedDataChunk, VectorResource},
};

pub enum TieredSearchResult {
    Chunk(DataChunk),
    Pointer(VectorResourcePointer),
    Resource(Box<dyn VectorResource>),
    TieredResource(Box<dyn TieredVectorResource>),
}

// These likely have to be their own traits because we'll have the basic
// VectorResources like DocumentVectorResouce and MapVectorResource, and then
// more specific ones like CodeFileVectorResource where you need to implement
// custom extract_tiered_results (probably think through more)
pub trait TieredVectorResource: VectorResource {
    fn extract_tiered_results(ret_chunks: &Vec<RetrievedDataChunk>) -> Vec<TieredSearchResult>;

    fn tiered_vector_search(&self, query: &str, num_of_results: usize) -> Vec<TieredSearchResult> {
        let mut results = Vec::new();

        // Call vector search on the vector resource
        let chunks = self.vector_resource().vector_search(query, num_of_results);

        // Extract tiered results
        let tiered_results = self.extract_tiered_results(&chunks);
        for result in tiered_results {
            match result {
                TieredSearchResult::Resource(resource) => {
                    // For any results that are vector resources call vector search
                    let resource_results = resource.vector_search(query, num_of_results);
                    results.extend(resource_results.into_iter().map(TieredSearchResult::Resource));
                }
                TieredSearchResult::TieredResource(tiered_resource) => {
                    // For any that are tiered vector resources call their tiered_vector_search
                    let tiered_resource_results = tiered_resource.tiered_vector_search(query, num_of_results);
                    results.extend(tiered_resource_results);
                }
                _ => results.push(result),
            }
        }

        results
    }
}

// This trait should allow implementing more complex tiered vector resources, such as
// CodeProjectVectorResource where by using tiered_vector_search inside of folded_vector_search,
// the implementer can decide how to "fold" the results down into a single string, which propagates upwards
// and gets all put together as all function/imports/definition vector resources are all Foldable as well
pub trait FoldableVectorResource: TieredVectorResource {
    fn folded_vector_search(&self, query: &str, num_of_results: usize) -> String;
}
