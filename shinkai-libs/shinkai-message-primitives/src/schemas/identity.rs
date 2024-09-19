use ed25519_dalek::VerifyingKey;
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};
use std::{fmt, net::SocketAddr};
use x25519_dalek::PublicKey as EncryptionPublicKey;

use crate::shinkai_message::shinkai_message_schemas::IdentityPermissions;
use crate::shinkai_utils::encryption::{encryption_public_key_to_string, encryption_public_key_to_string_ref};
use crate::shinkai_utils::signatures::{signature_public_key_to_string, signature_public_key_to_string_ref};

use super::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use super::shinkai_name::ShinkaiName;

#[derive(Debug, PartialEq, PartialOrd, Eq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdentityType {
    Global,
    Device,
    LLMProvider,
    Profile,
}

impl IdentityType {
    pub fn to_enum(s: &str) -> Option<Self> {
        match s {
            "global" => Some(IdentityType::Global),
            "device" => Some(IdentityType::Device),
            "agent" => Some(IdentityType::LLMProvider),
            "profile" => Some(IdentityType::Profile),
            _ => None,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            IdentityType::Global => "global",
            IdentityType::Device => "device",
            IdentityType::LLMProvider => "agent",
            IdentityType::Profile => "profile",
        }
        .to_owned()
    }

    pub fn to_standard(&self) -> Option<StandardIdentityType> {
        match self {
            Self::Global => Some(StandardIdentityType::Global),
            Self::Profile => Some(StandardIdentityType::Profile),
            _ => None, // Agent and Device types don't have a StandardIdentityType equivalent
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
    pub registration_name: String,
    pub device_identity_pk: String,
    pub device_encryption_pk: String,
    pub profile_identity_pk: String,
    pub profile_encryption_pk: String,
    pub identity_type: IdentityType,
    pub permission_type: IdentityPermissions,
}

#[derive(Clone)]
pub enum Identity {
    // IdentityType::Global or IdentityType::Profile
    Standard(StandardIdentity),
    // IdentityType::LLMProvider
    LLMProvider(SerializedLLMProvider),
    // IdentityType::Device
    Device(DeviceIdentity),
}

impl Identity {
    pub fn get_full_identity_name(&self) -> String {
        match self {
            Identity::Standard(std_identity) => std_identity.full_identity_name.clone().to_string(),
            Identity::LLMProvider(agent) => agent.full_identity_name.clone().to_string(),
            Identity::Device(device) => device.full_identity_name.clone().to_string(),
        }
    }

    pub fn has_admin_permissions(&self) -> bool {
        match self {
            Identity::Standard(std_identity) => std_identity.permission_type == IdentityPermissions::Admin,
            Identity::LLMProvider(_) => false, // Assuming LLM providers don't have admin permissions
            Identity::Device(device) => device.permission_type == IdentityPermissions::Admin,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StandardIdentity {
    pub full_identity_name: ShinkaiName,
    pub addr: Option<SocketAddr>,
    pub node_encryption_public_key: EncryptionPublicKey,
    pub node_signature_public_key: VerifyingKey,
    pub profile_encryption_public_key: Option<EncryptionPublicKey>,
    pub profile_signature_public_key: Option<VerifyingKey>,
    pub identity_type: StandardIdentityType,
    pub permission_type: IdentityPermissions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceIdentity {
    // This would include the profile name e.g. @@Alice.shinkai/profileName/myPhone
    pub full_identity_name: ShinkaiName,
    pub node_encryption_public_key: EncryptionPublicKey,
    pub node_signature_public_key: VerifyingKey,
    pub profile_encryption_public_key: EncryptionPublicKey,
    pub profile_signature_public_key: VerifyingKey,
    pub device_encryption_public_key: EncryptionPublicKey,
    pub device_signature_public_key: VerifyingKey,
    pub permission_type: IdentityPermissions,
}

impl DeviceIdentity {
    pub fn to_standard_identity(&self) -> Option<StandardIdentity> {
        let full_identity_name = self.full_identity_name.extract_profile().ok()?;

        Some(StandardIdentity {
            full_identity_name,
            addr: None,
            node_encryption_public_key: self.node_encryption_public_key,
            node_signature_public_key: self.node_signature_public_key,
            profile_encryption_public_key: Some(self.profile_encryption_public_key),
            profile_signature_public_key: Some(self.profile_signature_public_key),
            identity_type: StandardIdentityType::Profile,
            permission_type: self.permission_type.clone(),
        })
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
        s.serialize_field(
            "node_encryption_public_key",
            &encryption_public_key_to_string_ref(&self.node_encryption_public_key),
        )?;
        s.serialize_field(
            "node_signature_public_key",
            &signature_public_key_to_string_ref(&self.node_signature_public_key),
        )?;
        s.serialize_field(
            "profile_encryption_public_key",
            &self.profile_encryption_public_key.map(encryption_public_key_to_string),
        )?;
        s.serialize_field(
            "profile_signature_public_key",
            &self.profile_signature_public_key.map(signature_public_key_to_string),
        )?;
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
        node_signature_public_key: VerifyingKey,
        subidentity_encryption_public_key: Option<EncryptionPublicKey>,
        subidentity_signature_public_key: Option<VerifyingKey>,
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
        s.serialize_field(
            "node_encryption_public_key",
            &encryption_public_key_to_string_ref(&self.node_encryption_public_key),
        )?;
        s.serialize_field(
            "node_signature_public_key",
            &signature_public_key_to_string_ref(&self.node_signature_public_key),
        )?;
        s.serialize_field(
            "profile_encryption_public_key",
            &encryption_public_key_to_string(self.profile_encryption_public_key),
        )?;
        s.serialize_field(
            "profile_signature_public_key",
            &signature_public_key_to_string(self.profile_signature_public_key),
        )?;
        s.serialize_field(
            "device_encryption_public_key",
            &encryption_public_key_to_string(self.device_encryption_public_key),
        )?;
        s.serialize_field(
            "device_signature_public_key",
            &signature_public_key_to_string(self.device_signature_public_key),
        )?;
        s.serialize_field("permission_type", &self.permission_type)?;
        s.end()
    }
}

impl fmt::Display for DeviceIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let node_encryption_public_key = encryption_public_key_to_string(self.node_encryption_public_key);
        let node_signature_public_key = signature_public_key_to_string(self.node_signature_public_key);

        let profile_encryption_public_key = encryption_public_key_to_string_ref(&self.profile_encryption_public_key);
        let profile_signature_public_key = signature_public_key_to_string_ref(&self.profile_signature_public_key);
        let device_encryption_public_key = encryption_public_key_to_string_ref(&self.device_encryption_public_key);
        let device_signature_public_key = signature_public_key_to_string_ref(&self.device_signature_public_key);

        write!(f, "DeviceIdentity {{ full_identity_name: {}, node_encryption_public_key: {:?}, node_signature_public_key: {:?}, profile_encryption_public_key: {}, profile_signature_public_key: {}, device_encryption_public_key: {}, device_signature_public_key: {}, permission_type: {:?} }}",
            self.full_identity_name,
            node_encryption_public_key,
            node_signature_public_key,
            profile_encryption_public_key,
            profile_signature_public_key,
            device_encryption_public_key,
            device_signature_public_key,
            self.permission_type
        )
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Identity::Standard(std_identity) => {
                write!(f, "Standard({})", std_identity)
            }
            Identity::LLMProvider(agent) => {
                // Assuming you have implemented Debug for SerializedLLMProvider
                write!(f, "Agent({:?})", agent)
            }
            Identity::Device(device) => {
                let node_encryption_public_key = encryption_public_key_to_string(device.node_encryption_public_key);
                let node_signature_public_key = signature_public_key_to_string(device.node_signature_public_key);
                let profile_encryption_public_key =
                    encryption_public_key_to_string_ref(&device.profile_encryption_public_key);
                let profile_signature_public_key =
                    signature_public_key_to_string_ref(&device.profile_signature_public_key);
                let device_encryption_public_key =
                    encryption_public_key_to_string_ref(&device.device_encryption_public_key);
                let device_signature_public_key =
                    signature_public_key_to_string_ref(&device.device_signature_public_key);

                write!(f, "DeviceIdentity {{ full_identity_name: {}, node_encryption_public_key: {:?}, node_signature_public_key: {:?}, profile_encryption_public_key: {:?}, profile_signature_public_key: {:?}, device_encryption_public_key: {:?}, device_signature_public_key: {:?}, permission_type: {:?} }}",
                    device.full_identity_name,
                    node_encryption_public_key,
                    node_signature_public_key,
                    profile_encryption_public_key,
                    profile_signature_public_key,
                    device_encryption_public_key,
                    device_signature_public_key,
                    device.permission_type)
            }
        }
    }
}
