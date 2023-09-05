use super::document_resource::DocumentVectorResource;
use super::map_resource::MapVectorResource;
use super::vector_resource::VectorResource;
use crate::resources::resource_errors::VectorResourceError;
use serde_json::Value as JsonValue;
use std::str::FromStr;

/// The list of base/core VectorResource types which are fully
/// composable within one another
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BaseVectorResource {
    Document(DocumentVectorResource),
    Map(MapVectorResource),
}

impl BaseVectorResource {
    /// Converts into a Box<&dyn VectorResource>.
    /// Used to access all of the VectorResource trait's methods, ie.
    /// self.as_trait_object().vector_search(...);
    ///
    /// Note this is not a mutable reference, so do not use mutating methods.
    pub fn as_trait_object(&self) -> Box<&dyn VectorResource> {
        match self {
            BaseVectorResource::Document(resource) => Box::new(resource),
            BaseVectorResource::Map(resource) => Box::new(resource),
        }
    }

    /// Converts into a Box<&mut dyn VectorResource>, which provides ability
    /// to mutate the BaseVectorResource using the VectorResource trait's methods.
    pub fn as_trait_object_mut(&mut self) -> Box<&mut dyn VectorResource> {
        match self {
            BaseVectorResource::Document(resource) => Box::new(resource),
            BaseVectorResource::Map(resource) => Box::new(resource),
        }
    }

    /// Converts the BaseVectorResource into a JSON string (without the enum wrapping JSON)
    pub fn to_json(&self) -> Result<String, VectorResourceError> {
        self.as_trait_object().to_json()
    }

    /// Creates a BaseVectorResource from a JSON string
    pub fn from_json(json: &str) -> Result<Self, VectorResourceError> {
        let value: JsonValue = serde_json::from_str(json)?;

        match value.get("resource_base_type") {
            Some(serde_json::Value::String(resource_type)) => match VectorResourceBaseType::from_str(resource_type) {
                Ok(VectorResourceBaseType::Document) => {
                    let document_resource = DocumentVectorResource::from_json(json)?;
                    Ok(BaseVectorResource::Document(document_resource))
                }
                Ok(VectorResourceBaseType::Map) => {
                    let map_resource = MapVectorResource::from_json(json)?;
                    Ok(BaseVectorResource::Map(map_resource))
                }
                _ => Err(VectorResourceError::InvalidVectorResourceBaseType),
            },
            _ => Err(VectorResourceError::InvalidVectorResourceBaseType),
        }
    }

    /// Attempts to convert the BaseVectorResource into a DocumentVectorResource
    pub fn as_document_resource(&self) -> Result<DocumentVectorResource, VectorResourceError> {
        match self {
            BaseVectorResource::Document(resource) => Ok(resource.clone()),
            _ => Err(VectorResourceError::InvalidVectorResourceBaseType),
        }
    }

    /// Attempts to convert the BaseVectorResource into a MapVectorResource
    pub fn as_map_resource(&self) -> Result<MapVectorResource, VectorResourceError> {
        match self {
            BaseVectorResource::Map(resource) => Ok(resource.clone()),
            _ => Err(VectorResourceError::InvalidVectorResourceBaseType),
        }
    }

    /// Returns the base type of the VectorResource
    pub fn resource_base_type(&self) -> VectorResourceBaseType {
        self.as_trait_object().resource_base_type()
    }
}

impl From<DocumentVectorResource> for BaseVectorResource {
    fn from(resource: DocumentVectorResource) -> Self {
        BaseVectorResource::Document(resource)
    }
}

impl From<MapVectorResource> for BaseVectorResource {
    fn from(resource: MapVectorResource) -> Self {
        BaseVectorResource::Map(resource)
    }
}

/// Enum used for VectorResources to self-attest their base type.
///
/// `CustomUnsupported(s)` allows for devs to implement custom VectorResources that fulfill the trait,
/// but which aren't composable with any of the base resources (we are open to PRs for adding new base types as well).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum VectorResourceBaseType {
    Document,
    Map,
    CustomUnsupported(String),
}

impl VectorResourceBaseType {
    pub fn to_str(&self) -> &str {
        match self {
            VectorResourceBaseType::Document => "Document",
            VectorResourceBaseType::Map => "Map",
            VectorResourceBaseType::CustomUnsupported(s) => s,
        }
    }

    /// Check if the given resource type is one of the supported types.
    /// Does this by using to/from_str to reuse the `match`es and keep code cleaner.
    pub fn is_base_vector_resource(resource_base_type: VectorResourceBaseType) -> Result<(), VectorResourceError> {
        let resource_type_str = resource_base_type.to_str();
        match Self::from_str(resource_type_str) {
            Ok(_) => Ok(()),
            Err(_) => Err(VectorResourceError::InvalidVectorResourceBaseType),
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
