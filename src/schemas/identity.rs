use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::managers::agent_serialization::SerializedAgent;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde::ser::{Serializer, SerializeStruct};
use shinkai_message_wasm::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_wasm::shinkai_utils::encryption::{encryption_public_key_to_string, encryption_public_key_to_string_ref};
use shinkai_message_wasm::shinkai_utils::signatures::{signature_public_key_to_string, signature_public_key_to_string_ref};
use std::sync::Arc;
use std::{fmt, net::SocketAddr};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[derive(Debug, PartialEq, PartialOrd, Eq, Clone, Serialize, Deserialize)]
pub enum IdentityType {
    Global,
    Device,
    Agent,
    Profile,
}

impl IdentityType {
    pub fn to_enum(s: &str) -> Option<Self> {
        match s {
            "global" => Some(IdentityType::Global),
            "device" => Some(IdentityType::Device),
            "agent" => Some(IdentityType::Agent),
            "profile" => Some(IdentityType::Profile),
            _ => None,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            IdentityType::Global => "global",
            IdentityType::Device => "device",
            IdentityType::Agent => "agent",
            IdentityType::Profile => "profile",
        }
        .to_owned()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegistrationCode {
    pub code: String,
    pub profile_name: String,
    pub identity_pk: String,
    pub encryption_pk: String,
    pub permission_type: String,
}

#[derive(Debug, Clone)]
pub enum Identity {
    Standard(StandardIdentity),
    Agent(SerializedAgent),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StandardIdentity {
    pub full_identity_name: String,
    pub addr: Option<SocketAddr>,
    pub node_encryption_public_key: EncryptionPublicKey,
    pub node_signature_public_key: SignaturePublicKey,
    pub subidentity_encryption_public_key: Option<EncryptionPublicKey>,
    pub subidentity_signature_public_key: Option<SignaturePublicKey>,
    pub permission_type: IdentityType,
}



impl Serialize for StandardIdentity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("StandardIdentity", 7)?;
        s.serialize_field("full_identity_name", &self.full_identity_name)?;
        s.serialize_field("addr", &self.addr)?;
        s.serialize_field("node_encryption_public_key", &encryption_public_key_to_string_ref(&self.node_encryption_public_key))?;
        s.serialize_field("node_signature_public_key", &signature_public_key_to_string_ref(&self.node_signature_public_key))?;
        s.serialize_field("subidentity_encryption_public_key", &self.subidentity_encryption_public_key.map(encryption_public_key_to_string))?;
        s.serialize_field("subidentity_signature_public_key", &self.subidentity_signature_public_key.map(signature_public_key_to_string))?;
        s.serialize_field("permission_type", &self.permission_type)?;
        s.end()
    }
}

impl StandardIdentity {
    pub fn new(
        full_identity_name: String,
        addr: Option<SocketAddr>,
        node_encryption_public_key: EncryptionPublicKey,
        node_signature_public_key: SignaturePublicKey,
        subidentity_encryption_public_key: Option<EncryptionPublicKey>,
        subidentity_signature_public_key: Option<SignaturePublicKey>,
        identity_type: IdentityType,
    ) -> Self {
        // If Identity is of type Global or Agent, clear the subidentity keys
        let subidentity_encryption_public_key = if matches!(identity_type, IdentityType::Global | IdentityType::Agent) {
            None
        } else {
            subidentity_encryption_public_key
        };

        let subidentity_signature_public_key = if matches!(identity_type, IdentityType::Global | IdentityType::Agent) {
            None
        } else {
            subidentity_signature_public_key
        };

        Self {
            full_identity_name,
            addr,
            node_encryption_public_key,
            node_signature_public_key,
            subidentity_encryption_public_key,
            subidentity_signature_public_key,
            permission_type: identity_type,
        }
    }

    pub fn node_identity_name(&self) -> &str {
        self.full_identity_name
            .split('/')
            .next()
            .unwrap_or(&self.full_identity_name)
    }

    pub fn subidentity_name(&self) -> Option<&str> {
        let parts: Vec<&str> = self.full_identity_name.split('/').collect();
        if parts.len() > 1 {
            Some(parts[1])
        } else {
            None
        }
    }
}

impl fmt::Display for StandardIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let node_encryption_public_key = encryption_public_key_to_string(self.node_encryption_public_key);
        let node_signature_public_key = signature_public_key_to_string(self.node_signature_public_key);

        let subidentity_encryption_public_key = self
            .subidentity_encryption_public_key
            .as_ref()
            .map(encryption_public_key_to_string_ref)
            .unwrap_or_else(|| "None".to_string());
        let subidentity_signature_public_key = self
            .subidentity_signature_public_key
            .as_ref()
            .map(signature_public_key_to_string_ref)
            .unwrap_or_else(|| "None".to_string());

        write!(f, "NewIdentity {{ full_identity_name: {}, addr: {:?}, node_encryption_public_key: {:?}, node_signature_public_key: {:?}, subidentity_encryption_public_key: {}, subidentity_signature_public_key: {}, permission_type: {:?} }}",
            self.full_identity_name,
            self.addr,
            node_encryption_public_key,
            node_signature_public_key,
            subidentity_encryption_public_key,
            subidentity_signature_public_key,
            self.permission_type
        )
    }
}