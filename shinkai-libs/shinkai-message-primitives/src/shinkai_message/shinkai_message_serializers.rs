use super::shinkai_message::ShinkaiVersion;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

impl Serialize for ShinkaiVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let version = match *self {
            ShinkaiVersion::V1_0 => "V1_0",
            ShinkaiVersion::Unsupported => "Unsupported",
        };
        serializer.serialize_str(version)
    }
}

impl<'de> Deserialize<'de> for ShinkaiVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let version = String::deserialize(deserializer)?;
        Ok(match version.as_str() {
            "V1_0" => ShinkaiVersion::V1_0,
            _ => ShinkaiVersion::Unsupported,
        })
    }
}
