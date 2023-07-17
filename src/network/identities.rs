use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use regex::Regex;
use serde::{Serialize, Deserialize};
use std::{net::SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};
use crate::db::{ShinkaiMessageDB};
use crate::db::db_errors::{ShinkaiMessageDBError};

#[derive(Debug, PartialEq, PartialOrd, Eq, Clone)]
pub enum IdentityType {
    Global,
    Device,
    Agent,
}

impl IdentityType {
    pub fn to_enum(s: &str) -> Option<Self> {
        match s {
            "global" => Some(IdentityType::Global),
            "device" => Some(IdentityType::Device),
            "agent" => Some(IdentityType::Agent),
            _ => None,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            IdentityType::Global => "global",
            IdentityType::Device => "device",
            IdentityType::Agent => "agent",
        }.to_owned()
    }
}

#[derive(Clone, Debug)]
pub struct Identity {
    pub name: String,
    pub addr: Option<SocketAddr>,
    pub encryption_public_key: Option<EncryptionPublicKey>,
    pub signature_public_key: Option<SignaturePublicKey>,
    pub permission_type: IdentityType,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegistrationCode {
    pub code: String,
    pub profile_name: String,
    pub identity_pk: String,
    pub encryption_pk: String,
    pub permission_type: String,
}

impl Identity {
    pub fn new(
        name: String,
        encryption_public_key: Option<EncryptionPublicKey>,
        signature_public_key: Option<SignaturePublicKey>,
        permission_type: IdentityType,
    ) -> Self {
        Self {
            name,
            addr: None,
            encryption_public_key,
            signature_public_key,
            permission_type,
        }
    }
}

// pub struct IdentityManager {
//     identities: Vec<Identity>,
//     db: Arc<Mutex<ShinkaiMessageDB>>,
// }
// 
// impl IdentityManager {
//     pub async fn new(db: Arc<Mutex<ShinkaiMessageDB>>) -> Result<Self, Box<dyn std::error::Error>> {
//         let identities = {
//             let db = db.lock().await;
//             let identities_tuple_vec = db.load_all_sub_identities()?;
//             identities_tuple_vec.into_iter().map(|(name, encryption_public_key, signature_public_key, permission_type)| {
//                 Identity::new(name, Some(encryption_public_key), Some(signature_public_key), permission_type)
//             }).collect::<Vec<Identity>>()
//         };
//         Ok(Self { identities, db })
//     }

//     pub fn extract_subidentity(s: &str) -> String {
//         let re = Regex::new(r"@@[^/]+\.shinkai/(.+)").unwrap();
//         re.captures(s).and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
//             .unwrap_or_else(|| s.to_string())
//     } 

//     pub async fn add_subidentity(&mut self, identity: Identity) -> anyhow::Result<()> {
//         let mut db = self.db.lock().await;
//         let normalized_identity = Identity::new(
//             IdentityManager::extract_subidentity(&identity.name.clone()),
//             identity.encryption_public_key.clone(),
//             identity.signature_public_key.clone(),
//             identity.permission_type.clone(),
//         );
//         db.insert_sub_identity(normalized_identity.clone())?;
//         self.identities.push(normalized_identity);
//         Ok(())
//     }

//     pub async fn remove_subidentity(&mut self, name: &str) -> Result<(), ShinkaiMessageDBError> {
//         let mut db = self.db.lock().await;
//         db.remove_identity(name)?;
//         self.identities.retain(|i| i.name != name);
//         Ok(())
//     }
    
//     pub fn find_by_signature_key(&self, key: &SignaturePublicKey) -> Option<&Identity> {
//         self.identities
//             .iter()
//             .find(|identity| identity.signature_public_key.as_ref() == Some(key))
//     }

//     pub fn find_by_profile_name(&self, profile_name: &str) -> Option<&Identity> {
//         let normalized_profile_name = IdentityManager::extract_subidentity(profile_name);
//         self.identities.iter()
//             .find(|identity| identity.name == normalized_profile_name)
//     }

//     pub fn get_all_subidentities(&self) -> Vec<Identity> {
//         self.identities.clone()
//     }

//     pub fn update_socket_addr(
//         &mut self,
//         name: &str,
//         new_addr: Option<SocketAddr>,
//     ) -> Result<(), &'static str> {
//         match self
//             .identities
//             .iter_mut()
//             .find(|identity| identity.name == name)
//         {
//             Some(identity) => {
//                 identity.addr = new_addr;
//                 Ok(())
//             }
//             None => Err("SubIdentity not found"),
//         }
//     }
// }
