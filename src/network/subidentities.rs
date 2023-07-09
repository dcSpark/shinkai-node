use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use regex::Regex;
use serde::{Serialize, Deserialize};
use std::{net::SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};
use crate::db::{ShinkaiMessageDB, ShinkaiMessageDBError};

#[derive(Clone, Debug)]
pub struct Subidentity {
    pub name: String,
    pub addr: Option<SocketAddr>,
    pub encryption_public_key: Option<EncryptionPublicKey>,
    pub signature_public_key: Option<SignaturePublicKey>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegistrationCode {
    pub code: String,
    pub profile_name: String,
    pub identity_pk: String,
    pub encryption_pk: String,
}

impl Subidentity {
    pub fn new(
        name: String,
        encryption_public_key: Option<EncryptionPublicKey>,
        signature_public_key: Option<SignaturePublicKey>,
    ) -> Self {
        Self {
            name,
            addr: None,
            encryption_public_key,
            signature_public_key,
        }
    }
}

pub struct SubIdentityManager {
    identities: Vec<Subidentity>,
    db: Arc<Mutex<ShinkaiMessageDB>>,
}

impl SubIdentityManager {
    pub async fn new(db: Arc<Mutex<ShinkaiMessageDB>>) -> Result<Self, Box<dyn std::error::Error>> {
        let identities = {
            let db = db.lock().await;
            let identities_tuple_vec = db.load_all_sub_identities()?;
            identities_tuple_vec.into_iter().map(|(name, encryption_public_key, signature_public_key)| {
                Subidentity::new(name, Some(encryption_public_key), Some(signature_public_key))
            }).collect::<Vec<Subidentity>>()
        };
        Ok(Self { identities, db })
    }

    pub fn extract_subidentity(s: &str) -> String {
        let re = Regex::new(r"@@[^/]+\.shinkai/(.+)").unwrap();
        re.captures(s).and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .unwrap_or_else(|| s.to_string())
    } 

    pub async fn add_subidentity(&mut self, identity: Subidentity) -> anyhow::Result<()> {
        let mut db = self.db.lock().await;
        let normalized_identity = Subidentity::new(
            SubIdentityManager::extract_subidentity(&identity.name.clone()),
            identity.encryption_public_key.clone(),
            identity.signature_public_key.clone(),
        );
        db.insert_sub_identity(normalized_identity.clone())?;
        self.identities.push(normalized_identity);
        Ok(())
    }

    pub async fn remove_subidentity(&mut self, name: &str) -> Result<(), ShinkaiMessageDBError> {
        let mut db = self.db.lock().await;
        db.remove_identity(name)?;
        self.identities.retain(|i| i.name != name);
        Ok(())
    }
    
    pub fn find_by_signature_key(&self, key: &SignaturePublicKey) -> Option<&Subidentity> {
        self.identities
            .iter()
            .find(|identity| identity.signature_public_key.as_ref() == Some(key))
    }

    pub fn find_by_profile_name(&self, profile_name: &str) -> Option<&Subidentity> {
        let normalized_profile_name = SubIdentityManager::extract_subidentity(profile_name);
        self.identities.iter()
            .find(|identity| identity.name == normalized_profile_name)
    }

    pub fn get_all_subidentities(&self) -> Vec<Subidentity> {
        self.identities.clone()
    }

    pub fn update_socket_addr(
        &mut self,
        name: &str,
        new_addr: Option<SocketAddr>,
    ) -> Result<(), &'static str> {
        match self
            .identities
            .iter_mut()
            .find(|identity| identity.name == name)
        {
            Some(identity) => {
                identity.addr = new_addr;
                Ok(())
            }
            None => Err("SubIdentity not found"),
        }
    }
}
