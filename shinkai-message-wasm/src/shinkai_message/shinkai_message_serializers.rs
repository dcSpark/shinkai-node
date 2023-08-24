use core::panic;

use super::{
    shinkai_message::{
        EncryptedShinkaiBody, EncryptedShinkaiData, MessageBody, MessageData, ShinkaiBody, ShinkaiData, ShinkaiVersion,
    },
    shinkai_message_error::ShinkaiMessageError,
};
use serde::{de::Error, ser::SerializeMap};
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

// impl Serialize for MessageBody {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         let (is_encrypted, value) = match self {
//             MessageBody::Encrypted(body) => (true, serde_json::to_value(body).unwrap()),
//             MessageBody::Unencrypted(body) => (false, serde_json::to_value(body).unwrap()),
//         };
//         let mut map = serializer.serialize_map(Some(2))?;
//         map.serialize_entry("type", &is_encrypted)?;
//         map.serialize_entry("value", &value)?;
//         map.end()
//     }
// }

// impl<'de> Deserialize<'de> for MessageBody {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         let map: serde_json::Map<String, serde_json::Value> = Deserialize::deserialize(deserializer)?;
//         let is_encrypted: bool = serde_json::from_value(map["type"].clone()).unwrap();
//         if is_encrypted {
//             let body: EncryptedShinkaiBody = serde_json::from_value(map["value"].clone()).unwrap();
//             Ok(MessageBody::Encrypted(body))
//         } else {
//             let body: ShinkaiBody = serde_json::from_value(map["value"].clone()).unwrap();
//             Ok(MessageBody::Unencrypted(body))
//         }
//     }
// }

// impl Serialize for MessageData {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         let (is_encrypted, value) = match self {
//             MessageData::Encrypted(data) => (true, serde_json::to_value(data).unwrap()),
//             MessageData::Unencrypted(data) => (false, serde_json::to_value(data).unwrap()),
//         };
//         let mut map = serializer.serialize_map(Some(2))?;
//         map.serialize_entry("type", &is_encrypted)?;
//         map.serialize_entry("value", &value)?;
//         map.end()
//     }
// }

// impl<'de> Deserialize<'de> for MessageData {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         let map: serde_json::Map<String, serde_json::Value> = Deserialize::deserialize(deserializer)?;
//         let is_encrypted: bool = serde_json::from_value(map["type"].clone()).unwrap();
//         if is_encrypted {
//             let data: EncryptedShinkaiData = serde_json::from_value(map["value"].clone()).unwrap();
//             Ok(MessageData::Encrypted(data))
//         } else {
//             let data: ShinkaiData = serde_json::from_value(map["value"].clone()).unwrap();
//             Ok(MessageData::Unencrypted(data))
//         }
//     }
// }