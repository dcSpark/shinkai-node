use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::managers::agent_serialization::SerializedAgent;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde::ser::{Serializer, SerializeStruct};
use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;
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

    pub fn to_standard(&self) -> Option<StandardIdentityType> {
        match self {
            Self::Global => Some(StandardIdentityType::Global),
            Self::Profile => Some(StandardIdentityType::Profile),
            _ => None,  // Agent and Device types don't have a StandardIdentityType equivalent
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Eq, Clone, Serialize, Deserialize)]
pub enum StandardIdentityType {
    Global,
    Profile,
}

impl StandardIdentityType {
    pub fn to_enum(s: &str) -> Option<Self> {
        match s {
            "global" => Some(StandardIdentityType::Global),
            "profile" => Some(StandardIdentityType::Profile),
            _ => None,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            StandardIdentityType::Global => "global",
            StandardIdentityType::Profile => "profile",
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
    pub identity_type: IdentityType,
    pub permission_type: IdentityPermissions,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum IdentityPermissions {
    Admin, // can create and delete other profiles
    Standard, // can add / remove devices
    None, // none of the above
}

#[derive(Debug, Clone)]
pub enum Identity {
    // IdentityType::Global or IdentityType::Profile
    Standard(StandardIdentity),
    // IdentityType::Agent
    Agent(SerializedAgent),
    // IdentityType::Device
    Device(DeviceIdentity),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StandardIdentity {
    pub full_identity_name: ShinkaiName,
    pub addr: Option<SocketAddr>,
    pub node_encryption_public_key: EncryptionPublicKey,
    pub node_signature_public_key: SignaturePublicKey,
    pub profile_encryption_public_key: Option<EncryptionPublicKey>,
    pub profile_signature_public_key: Option<SignaturePublicKey>,
    pub identity_type: StandardIdentityType,
    pub permission_type: IdentityPermissions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceIdentity {
    // This would include the profile name e.g. @@Alice.shinkai/profileName/myPhone
    pub full_identity_name: ShinkaiName,
    pub node_encryption_public_key: EncryptionPublicKey,
    pub node_signature_public_key: SignaturePublicKey,
    pub profile_encryption_public_key: Option<EncryptionPublicKey>,
    pub profile_signature_public_key: Option<SignaturePublicKey>,
    pub device_signature_public_key: Option<SignaturePublicKey>,
    pub permission_type: IdentityPermissions,
}

impl DeviceIdentity {
    pub fn to_standard_identity(&self) -> Option<StandardIdentity> {
        let full_identity_name = self.full_identity_name.extract_profile().ok()?;
        
        Some(StandardIdentity {
            full_identity_name,
            addr: None,
            node_encryption_public_key: self.node_encryption_public_key.clone(),
            node_signature_public_key: self.node_signature_public_key.clone(),
            profile_encryption_public_key: self.profile_encryption_public_key.clone(),
            profile_signature_public_key: self.profile_signature_public_key.clone(),
            identity_type: StandardIdentityType::Profile,
            permission_type: self.permission_type.clone(),
        })
    }
}

impl IdentityPermissions {
    pub fn from_slice(slice: &[u8]) -> Self {
        let s = std::str::from_utf8(slice).unwrap();
        match s {
            "admin" => Self::Admin,
            "standard" => Self::Standard,
            _ => Self::None,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Admin => b"admin",
            Self::Standard => b"standard",
            Self::None => b"none",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Admin" => Some(Self::Admin),
            "Standard" => Some(Self::Standard),
            "None" => Some(Self::None),
            _ => None,
        }
    }
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
        s.serialize_field("profile_encryption_public_key", &self.profile_encryption_public_key.map(encryption_public_key_to_string))?;
        s.serialize_field("profile_signature_public_key", &self.profile_signature_public_key.map(signature_public_key_to_string))?;
        s.serialize_field("identity_type", &self.identity_type)?;
        s.serialize_field("permission_type", &self.permission_type)?;
        s.end()
    }
}

impl StandardIdentity {
    pub fn new(
        full_identity_name: ShinkaiName,
        addr: Option<SocketAddr>,
        node_encryption_public_key: EncryptionPublicKey,
        node_signature_public_key: SignaturePublicKey,
        subidentity_encryption_public_key: Option<EncryptionPublicKey>,
        subidentity_signature_public_key: Option<SignaturePublicKey>,
        identity_type: StandardIdentityType,
        permission_type: IdentityPermissions,
    ) -> Self {
        // If Identity is of type Global or Agent, clear the subidentity keys
        let subidentity_encryption_public_key = if matches!(identity_type, StandardIdentityType::Global) {
            None
        } else {
            subidentity_encryption_public_key
        };

        let subidentity_signature_public_key = if matches!(identity_type, StandardIdentityType::Global) {
            None
        } else {
            subidentity_signature_public_key
        };

        Self {
            full_identity_name,
            addr,
            node_encryption_public_key,
            node_signature_public_key,
            profile_encryption_public_key: subidentity_encryption_public_key,
            profile_signature_public_key: subidentity_signature_public_key,
            identity_type,
            permission_type,
        }
    }
}

impl fmt::Display for StandardIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let node_encryption_public_key = encryption_public_key_to_string(self.node_encryption_public_key);
        let node_signature_public_key = signature_public_key_to_string(self.node_signature_public_key);

        let profile_encryption_public_key = self
            .profile_encryption_public_key
            .as_ref()
            .map(encryption_public_key_to_string_ref)
            .unwrap_or_else(|| "None".to_string());
        let profile_signature_public_key = self
            .profile_signature_public_key
            .as_ref()
            .map(signature_public_key_to_string_ref)
            .unwrap_or_else(|| "None".to_string());

        write!(f, "NewIdentity {{ full_identity_name: {}, addr: {:?}, node_encryption_public_key: {:?}, node_signature_public_key: {:?}, profile_encryption_public_key: {}, profile_signature_public_key: {}, identity_type: {:?}, permission_type: {:?} }}",
            self.full_identity_name,
            self.addr,
            node_encryption_public_key,
            node_signature_public_key,
            profile_encryption_public_key,
            profile_signature_public_key,
            self.identity_type,
            self.permission_type
        )
    }
}

impl Serialize for DeviceIdentity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("DeviceIdentity", 6)?;
        s.serialize_field("full_identity_name", &self.full_identity_name)?;
        s.serialize_field("node_encryption_public_key", &encryption_public_key_to_string_ref(&self.node_encryption_public_key))?;
        s.serialize_field("node_signature_public_key", &signature_public_key_to_string_ref(&self.node_signature_public_key))?;
        s.serialize_field("profile_encryption_public_key", &self.profile_encryption_public_key.map(encryption_public_key_to_string))?;
        s.serialize_field("profile_signature_public_key", &self.profile_signature_public_key.map(signature_public_key_to_string))?;
        s.serialize_field("device_signature_public_key", &self.device_signature_public_key.map(signature_public_key_to_string))?;
        s.serialize_field("permission_type", &self.permission_type)?;
        s.end()
    }
}

impl fmt::Display for DeviceIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let node_encryption_public_key = encryption_public_key_to_string(self.node_encryption_public_key);
        let node_signature_public_key = signature_public_key_to_string(self.node_signature_public_key);

        let profile_encryption_public_key = self
            .profile_encryption_public_key
            .as_ref()
            .map(encryption_public_key_to_string_ref)
            .unwrap_or_else(|| "None".to_string());
        let profile_signature_public_key = self
            .profile_signature_public_key
            .as_ref()
            .map(signature_public_key_to_string_ref)
            .unwrap_or_else(|| "None".to_string());
        let device_signature_public_key = self
            .device_signature_public_key
            .as_ref()
            .map(signature_public_key_to_string_ref)
            .unwrap_or_else(|| "None".to_string());

        write!(f, "DeviceIdentity {{ full_identity_name: {}, node_encryption_public_key: {:?}, node_signature_public_key: {:?}, profile_encryption_public_key: {}, profile_signature_public_key: {}, device_signature_public_key: {}, permission_type: {:?} }}",
            self.full_identity_name,
            node_encryption_public_key,
            node_signature_public_key,
            profile_encryption_public_key,
            profile_signature_public_key,
            device_signature_public_key,
            self.permission_type
        )
    }
}

impl fmt::Display for IdentityPermissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admin => write!(f, "Admin"),
            Self::Standard => write!(f, "Standard"),
            Self::None => write!(f, "None"),
        }
    }
}
