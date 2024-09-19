use std::{fmt, time::SystemTime};
use std::hash::{Hash, Hasher};

use chrono::{DateTime, Utc};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

use super::shinkai_subscription_req::FolderSubscription;

pub type FileMapPath = String;

#[derive(Debug, Clone, PartialEq)]
pub enum SubscriptionStatus {
    NotStarted,
    Syncing,
    WaitingForLinks,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileLink {
    pub link: String,
    pub path: String,
    pub last_8_hash: String,
    #[serde(serialize_with = "serialize_system_time")]
    #[serde(deserialize_with = "deserialize_system_time")]
    pub expiration: SystemTime,
}

fn serialize_system_time<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let datetime: DateTime<Utc> = (*time).into();
    serializer.serialize_str(&datetime.to_rfc3339())
}

fn deserialize_system_time<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
where
    D: Deserializer<'de>,
{
    struct SystemTimeVisitor;

    impl<'de> Visitor<'de> for SystemTimeVisitor {
        type Value = SystemTime;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string formatted as ISO8601")
        }

        fn visit_str<E>(self, value: &str) -> Result<SystemTime, E>
        where
            E: de::Error,
        {
            DateTime::parse_from_rfc3339(value)
                .map_err(de::Error::custom)
                .map(|dt| dt.with_timezone(&Utc).into())
        }
    }

    deserializer.deserialize_str(SystemTimeVisitor)
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    Sync(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderSubscriptionWithPath {
    pub path: String,
    pub folder_subscription: FolderSubscription,
}

impl Hash for FolderSubscriptionWithPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Only the path field is used for hashing
        self.path.hash(state);
    }
}

