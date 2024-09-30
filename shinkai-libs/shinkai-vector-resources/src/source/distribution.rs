use crate::vector_resource::VRPath;
use chrono::{DateTime, Utc};
use utoipa::ToSchema;

pub type ShinkaiNameString = String;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ToSchema)]
/// Info about where the source data was acquired from, and when it was originally distributed
pub struct DistributionInfo {
    pub origin: Option<DistributionOrigin>,
    #[schema(value_type = String, format = Date)]
    pub datetime: Option<DateTime<Utc>>,
}

impl DistributionInfo {
    /// Creates a new instance of DistributionInfo with specified origin and datetime
    pub fn new(origin: Option<DistributionOrigin>, datetime: Option<DateTime<Utc>>) -> Self {
        Self { origin, datetime }
    }

    /// Creates a new instance of DistributionInfo with auto-detecting origin based on file name
    pub fn new_auto(file_name: &str, datetime: Option<DateTime<Utc>>) -> Self {
        let origin = DistributionOrigin::new_auto(file_name);

        Self { origin, datetime }
    }

    /// Creates a new, empty instance of DistributionInfo with no origin and no datetime
    pub fn new_empty() -> Self {
        Self {
            origin: None,
            datetime: None,
        }
    }
}

/// The origin where the original data was acquired from.
/// Based on source file that was used to create the VR if one exists (ie. pdf/webpage), or based on the VR where/when it was released
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ToSchema)]
pub enum DistributionOrigin {
    Uri(String),
    #[schema(value_type = (String, VRPath))]
    ShinkaiNode((ShinkaiNameString, VRPath)),
    Other(String),
}

impl DistributionOrigin {
    /// Creates a new instance with auto-detecting origin based on input name/source string
    pub fn new_auto(input: &str) -> Option<Self> {
        if input.starts_with("http://")
            || input.starts_with("https://")
            || input.starts_with("ipfs://")
            || input.starts_with("ar://")
        {
            Some(DistributionOrigin::Uri(input.to_string()))
        } else if input.starts_with("@@") {
            let parts: Vec<&str> = input.splitn(2, '/').collect();
            if parts.len() == 2 {
                let name_string = parts[0].to_string();
                let vr_path_part = parts[1];
                if let Some(vr_path_index) = vr_path_part.find("/vec_fs") {
                    let vr_path = vr_path_part[vr_path_index..].to_string();
                    if let Ok(path) = VRPath::from_string(&vr_path) {
                        return Some(DistributionOrigin::ShinkaiNode((name_string, path)));
                    }
                }
            }
            None
        } else {
            None
        }
    }

    // Converts the DistributionOrigin to a JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    // Creates a DistributionOrigin from a JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
