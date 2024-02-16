use crate::vector_resource::VRPath;

pub type ShinkaiNameString = String;

/// The origin where a VectorResource was downloaded/acquired from before it arrived
/// in the node's VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DistributionOrigin {
    Uri(String),
    ShinkaiNode((ShinkaiNameString, VRPath)),
    Other(String),
    None,
}

impl DistributionOrigin {
    // Converts the DistributionOrigin to a JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    // Creates a DistributionOrigin from a JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
