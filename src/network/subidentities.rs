use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use serde::{Serialize, Deserialize};
use std::{net::SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::db::{ShinkaiMessageDB, ShinkaiMessageDBError};

#[derive(Clone)]
pub struct SubIdentity {
    pub name: String,
    pub addr: Option<SocketAddr>,
    pub encryption_public_key: Option<EncryptionPublicKey>,
    pub signature_public_key: Option<SignaturePublicKey>,
}

#[derive(Serialize, Deserialize)]
pub struct RegistrationCode {
    pub code: String,
    pub profile_name: String,
    pub identity_pk: String,
    pub encryption_pk: String,
}

impl SubIdentity {
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
    identities: Vec<SubIdentity>,
    db: Arc<Mutex<ShinkaiMessageDB>>,
}

impl SubIdentityManager {
    pub async fn new(db: Arc<Mutex<ShinkaiMessageDB>>) -> Result<Self, Box<dyn std::error::Error>> {
        let identities = {
            let db = db.lock().await;
            let identities_tuple_vec = db.load_all_sub_identities()?;
            identities_tuple_vec.into_iter().map(|(name, encryption_public_key, signature_public_key)| {
                SubIdentity::new(name, Some(encryption_public_key), Some(signature_public_key))
            }).collect::<Vec<SubIdentity>>()
        };
        Ok(Self { identities, db })
    }

    pub async fn add_identity(&mut self, identity: SubIdentity) -> Result<(), Box<dyn std::error::Error>> {
        let mut db = self.db.lock().await;
        db.insert_sub_identity(identity.clone())?;
        self.identities.push(identity);
        Ok(())
    }

    pub async fn remove_identity(&mut self, name: &str) -> Result<(), ShinkaiMessageDBError> {
        let mut db = self.db.lock().await;
        db.remove_identity(name)?;
        self.identities.retain(|i| i.name != name);
        Ok(())
    }
    
    pub fn find_by_signature_key(&self, key: &SignaturePublicKey) -> Option<&SubIdentity> {
        self.identities
            .iter()
            .find(|identity| identity.signature_public_key.as_ref() == Some(key))
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
