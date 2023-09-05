use super::document_resource::DocumentVectorResource;
use super::map_resource::MapVectorResource;
use super::vector_resource::VectorResource;
use crate::resources::data_tags::{DataTag, DataTagIndex};
use crate::resources::embeddings::*;
use crate::resources::resource_errors::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

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
    /// self.as_trait_object().vector_search(...);
    pub fn as_trait_object(&self) -> Box<dyn VectorResource> {
        match self {
            BaseVectorResource::Document(resource) => Box::new(resource.clone()),
            BaseVectorResource::Map(resource) => Box::new(resource.clone()),
        }
    }

}

/// Enum used for all VectorResources to self-attest their base type.
/// Used primarily when dealing with Trait objects, and self-attesting
/// JSON serialized VectorResources
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum VectorResourceBaseType {
    Document,
    Map,
}

impl VectorResourceBaseType {
    pub fn to_str(&self) -> &str {
        match self {
            VectorResourceBaseType::Document => "Document",
            VectorResourceBaseType::Map => "Map",
        }
    }
}

impl FromStr for VectorResourceBaseType {
    type Err = VectorResourceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Document" => Ok(VectorResourceBaseType::Document),
            "Map" => Ok(VectorResourceBaseType::Map),
            _ => Err(VectorResourceError::InvalidVectorResourceBaseType),
        }
    }
}
