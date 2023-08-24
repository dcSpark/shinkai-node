use super::shinkai_message::{
    EncryptedShinkaiBody, EncryptedShinkaiData, MessageBody, MessageData, ShinkaiBody, ShinkaiData, ShinkaiVersion,
};
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

impl Serialize for MessageBody {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            MessageBody::Encrypted(body) => {
                let s = format!("encrypted:{}", body.content);
                serializer.serialize_str(&s)
            }
            MessageBody::Unencrypted(body) => {
                let encoded = bincode::serialize(body).unwrap();
                serializer.serialize_bytes(&encoded)
            }
        }
    }
}

impl<'de> Deserialize<'de> for MessageBody {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split(':').collect();
        match parts.get(0) {
            Some(&"encrypted") => {
                let content = parts.get(1).unwrap_or(&"");
                Ok(MessageBody::Encrypted(EncryptedShinkaiBody {
                    content: content.to_string(),
                }))
            }
            Some(&"unencrypted") => {
                let bytes = parts.get(1).unwrap_or(&"").as_bytes();
                let decoded: ShinkaiBody = bincode::deserialize(bytes).unwrap();
                Ok(MessageBody::Unencrypted(decoded))
            }
            _ => Err(serde::de::Error::custom("Unexpected variant")),
        }
    }
}

impl Serialize for MessageData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            MessageData::Encrypted(data) => {
                let s = format!("encrypted:{}", data.content);
                serializer.serialize_str(&s)
            }
            MessageData::Unencrypted(data) => {
                let encoded = bincode::serialize(data).unwrap();
                let s = format!("unencrypted:{}", hex::encode(&encoded));
                serializer.serialize_str(&s)
            }
        }
    }
}

impl<'de> Deserialize<'de> for MessageData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split(':').collect();
        match parts.get(0) {
            Some(&"encrypted") => {
                let content = parts.get(1).unwrap_or(&"");
                Ok(MessageData::Encrypted(EncryptedShinkaiData {
                    content: content.to_string(),
                }))
            }
            Some(&"unencrypted") => {
                let hex_string = parts.get(1).unwrap_or(&"");
                let bytes = hex::decode(hex_string).unwrap();
                let decoded: ShinkaiData = bincode::deserialize(&bytes).unwrap();
                Ok(MessageData::Unencrypted(decoded))
            }
            _ => Err(serde::de::Error::custom("Unexpected variant")),
        }
    }
}
