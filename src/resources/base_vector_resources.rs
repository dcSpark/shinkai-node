use super::document_resource::DocumentVectorResource;
use super::map_resource::MapVectorResource;
use super::vector_resource::VectorResource;
use crate::resources::data_tags::{DataTag, DataTagIndex};
use crate::resources::embeddings::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The list of base/core VectorResource types which are fully
/// composable within one another
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BaseVectorResource {
    Document(DocumentVectorResource),
    Map(MapVectorResource),
}

impl BaseVectorResource {
    /// Converts the BaseVectorResource into a Box<dyn VectorResource>
    /// Used to get access to all of the trait's methods, ie.
    /// self.trait_object().vector_search(...);
    pub fn trait_object(&self) -> Box<dyn VectorResource> {
        match self {
            BaseVectorResource::Document(resource) => Box::new(resource.clone()),
            BaseVectorResource::Map(resource) => Box::new(resource.clone()),
        }
    }
}
