use serde::{Deserialize, Serialize};
use shinkai_vector_resources::{
    base_vector_resources::BaseVectorResource, source::VRSource, vector_resource_types::VectorResourcePointer,
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LocalScopeEntry {
    pub resource: BaseVectorResource,
    pub source: VRSource,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DBScopeEntry {
    pub resource_pointer: VectorResourcePointer,
    pub source: VRSource,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LocalScope {
    pub entries: Vec<LocalScopeEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DBScope {
    pub entries: Vec<DBScopeEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
/// Job's scope which includes both local entries (source/vector resource stored locally only in job)
/// and DB entries (source/vector resource stored in the DB, accessible to all jobs)
pub struct JobScope {
    pub local: LocalScope,
    pub database: DBScope,
}

impl JobScope {
    pub fn new(local: LocalScope, database: DBScope) -> Self {
        Self { local, database }
    }

    pub fn new_default() -> Self {
        Self {
            local: LocalScope { entries: Vec::new() },
            database: DBScope { entries: Vec::new() },
        }
    }

    pub fn to_bytes(&self) -> serde_json::Result<Vec<u8>> {
        let j = serde_json::to_string(self)?;
        Ok(j.into_bytes())
    }

    pub fn from_bytes(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }

    pub fn from_json_str(s: &str) -> serde_json::Result<Self> {
        let deserialized: Self = serde_json::from_str(s)?;
        Ok(deserialized)
    }

    pub fn to_json_str(&self) -> serde_json::Result<String> {
        let json_str = serde_json::to_string(self)?;
        Ok(json_str)
    }
}
