use crate::vector_resource::VRPath;
use chrono::{DateTime, Utc};

pub type ShinkaiNameString = String;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
/// Info about where the source data was acquired from, and when it was originally distributed (aka. release origin/date)
pub struct DistributionInfo {
    pub origin: Option<DistributionOrigin>,
    pub release_datetime: Option<DateTime<Utc>>,
}

impl DistributionInfo {
    /// Creates a new instance of DistributionInfo with specified origin and release_datetime
    pub fn new(origin: Option<DistributionOrigin>, release_datetime: Option<DateTime<Utc>>) -> Self {
        Self {
            origin,
            release_datetime,
        }
    }

    /// Creates a new, empty instance of DistributionInfo with no origin and no release_datetime
    pub fn new_empty() -> Self {
        Self {
            origin: None,
            release_datetime: None,
        }
    }
}

/// The origin where the original data was acquired from.
/// Based on source file that was used to create the VR if one exists (ie. pdf/webpage), or based on the VR where/when it was released
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
