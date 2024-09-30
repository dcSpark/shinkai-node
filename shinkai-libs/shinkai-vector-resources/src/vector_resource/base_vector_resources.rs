use super::vector_resource::VectorResource;
use super::{DocumentVectorResource, MapVectorResource, VRKai};
use crate::resource_errors::VRError;
use crate::vector_resource::OrderedVectorResource;
use serde_json::Value as JsonValue;
use std::str::FromStr;
use utoipa::ToSchema;

/// The list of base/core VectorResource types which are fully
/// composable within one another
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ToSchema)]
pub enum BaseVectorResource {
    Document(DocumentVectorResource),
    Map(MapVectorResource),
}

impl BaseVectorResource {
    /// Converts into a Box<&dyn VectorResource>.
    /// Used to access all of the VectorResource trait's methods, ie.
    /// self.as_trait_object().vector_search(...);
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

    //// Attempts to cast into a OrderedVectorResource. Fails if
    /// the Resource does not support the OrderedVectorResource trait.
    pub fn as_ordered_vector_resource(&self) -> Result<&dyn OrderedVectorResource, VRError> {
        self.as_trait_object().as_ordered_vector_resource()
    }

    //// Attempts to cast into a OrderedVectorResource. Fails if
    /// the Resource does not support the OrderedVectorResource trait.
    pub fn as_ordered_vector_resource_mut(&mut self) -> Result<&mut dyn OrderedVectorResource, VRError> {
        self.as_trait_object_mut().as_ordered_vector_resource_mut()
    }

    /// Converts the BaseVectorResource into a VRKai instance using the previously defined new method.
    pub fn to_vrkai(self) -> VRKai {
        VRKai::new(self, None)
    }

    /// Converts the BaseVectorResource into a JSON string (without the enum wrapping JSON)
    pub fn to_json(&self) -> Result<String, VRError> {
        self.as_trait_object().to_json()
    }

    /// Converts the BaseVectorResource into a JSON Value (without the enum wrapping JSON)
    pub fn to_json_value(&self) -> Result<serde_json::Value, VRError> {
        self.as_trait_object().to_json_value()
    }

    /// Creates a BaseVectorResource from a JSON string
    pub fn from_json(json: &str) -> Result<Self, VRError> {
        let value: JsonValue = serde_json::from_str(json)?;

        match value.get("resource_base_type") {
            Some(serde_json::Value::String(resource_type)) => match VRBaseType::from_str(resource_type) {
                Ok(VRBaseType::Document) => {
                    let document_resource = DocumentVectorResource::from_json(json)?;
                    Ok(BaseVectorResource::Document(document_resource))
                }
                Ok(VRBaseType::Map) => {
                    let map_resource = MapVectorResource::from_json(json)?;
                    Ok(BaseVectorResource::Map(map_resource))
                }
                _ => Err(VRError::InvalidVRBaseType),
            },
            _ => Err(VRError::InvalidVRBaseType),
        }
    }

    /// Attempts to convert the BaseVectorResource into a DocumentVectorResource
    pub fn as_document_resource(&mut self) -> Result<&mut DocumentVectorResource, VRError> {
        match self {
            BaseVectorResource::Document(resource) => Ok(resource),
            _ => Err(VRError::InvalidVRBaseType),
        }
    }

    /// Attempts to convert the BaseVectorResource into a MapVectorResource
    pub fn as_map_resource(&mut self) -> Result<&mut MapVectorResource, VRError> {
        match self {
            BaseVectorResource::Map(resource) => Ok(resource),
            _ => Err(VRError::InvalidVRBaseType),
        }
    }

    /// Attempts to convert the BaseVectorResource into a DocumentVectorResource
    pub fn as_document_resource_cloned(&self) -> Result<DocumentVectorResource, VRError> {
        match self {
            BaseVectorResource::Document(resource) => Ok(resource.clone()),
            _ => Err(VRError::InvalidVRBaseType),
        }
    }

    /// Attempts to convert the BaseVectorResource into a MapVectorResource
    pub fn as_map_resource_cloned(&self) -> Result<MapVectorResource, VRError> {
        match self {
            BaseVectorResource::Map(resource) => Ok(resource.clone()),
            _ => Err(VRError::InvalidVRBaseType),
        }
    }

    /// Returns the base type of the VectorResource
    pub fn resource_base_type(&self) -> VRBaseType {
        self.as_trait_object().resource_base_type()
    }

    pub fn resource_contents_by_hierarchy_to_string(&self) -> String {
        self.as_trait_object()
            .retrieve_all_nodes_contents_by_hierarchy(None, false, false, false)
            .join("\n")
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ToSchema)]
pub enum VRBaseType {
    Document,
    Map,
    CustomUnsupported(String),
}

impl VRBaseType {
    pub fn to_str(&self) -> &str {
        match self {
            VRBaseType::Document => "Document",
            VRBaseType::Map => "Map",
            VRBaseType::CustomUnsupported(s) => s,
        }
    }

    /// Check if the given resource type is one of the supported types.
    /// Does this by using to/from_str to reuse the `match`es and keep code cleaner.
    pub fn is_base_vector_resource(resource_base_type: VRBaseType) -> Result<(), VRError> {
        let resource_type_str = resource_base_type.to_str();
        match Self::from_str(resource_type_str) {
            Ok(_) => Ok(()),
            Err(_) => Err(VRError::InvalidVRBaseType),
        }
    }
}

impl FromStr for VRBaseType {
    type Err = VRError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Document" => Ok(VRBaseType::Document),
            "Map" => Ok(VRBaseType::Map),
            _ => Err(VRError::InvalidVRBaseType),
        }
    }
}
