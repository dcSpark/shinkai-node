use std::{collections::HashMap, net::SocketAddr};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use lazy_static::lazy_static;

use crate::shinkai_message::{signatures::{string_to_signature_public_key, signature_public_key_to_string}, encryption::string_to_encryption_public_key};

#[derive(Debug)]
pub struct ExternalProfileData {
    pub node_identity_name: String,
    pub addr: SocketAddr,
    pub signature_public_key: SignaturePublicKey,
    pub encryption_public_key: EncryptionPublicKey,
}

lazy_static! {
    static ref IDENTITY_MAP: HashMap<String, ExternalProfileData> = {
        let mut m = HashMap::new();
        m.insert("@@node1.shinkai".to_string(), ExternalProfileData {
            node_identity_name: "@@node1.shinkai".to_string(),
            addr: SocketAddr::from(([127, 0, 0, 1], 8080)),
            signature_public_key: string_to_signature_public_key("9d7nvacMcG9kXpSMidcTRkKiAVtmkz8PAjSRXVA7HhwP").expect("Failed to parse public key"),
            encryption_public_key: string_to_encryption_public_key("9BUoYQYq7K38mkk61q8aMH9kD9fKSVL1Fib7FbH6nUkQ").expect("Failed to parse public key"),
        });
        m.insert("@@node2.shinkai".to_string(), ExternalProfileData {
            node_identity_name: "@@node2.shinkai".to_string(),
            addr: SocketAddr::from(([127, 0, 0, 1], 8081)),
            signature_public_key: string_to_signature_public_key("9NMC5fPm574jhjjdBe5oHfZa4DfAwrY8W8iJAjBafF82").expect("Failed to parse public key"),
            encryption_public_key: string_to_encryption_public_key("8NT3CZR16VApT1B5zhinbAdqAvt8QkqMXEiojeFaGdgV").expect("Failed to parse public key"),
        });
        m.insert("@@node3.shinkai".to_string(), ExternalProfileData {
            node_identity_name: "@@node3.shinkai".to_string(),
            addr: SocketAddr::from(([127, 0, 0, 1], 8082)),
            signature_public_key: string_to_signature_public_key("7iq1nvNfacJY3TGtLWJ7s2Weq9mV3rKWHtz6J3qVQyMf").expect("Failed to parse public key"),
            encryption_public_key: string_to_encryption_public_key("4PwpCXwBuZKhyBAsf2CuZwapotvXiHSq94kWcLLSxtcG").expect("Failed to parse public key"),
        });
        m
    };
}

pub fn external_identity_to_identity_pk(global_identity: String) -> Result<ExternalProfileData, &'static str> {
    match IDENTITY_MAP.get(&global_identity) {
        Some(data) => Ok(ExternalProfileData {
            node_identity_name: data.node_identity_name.clone(),
            addr: data.addr,
            signature_public_key: data.signature_public_key.clone(),
            encryption_public_key: data.encryption_public_key.clone(),
        }),
        None => Err("Unrecognized global identity"),
    }
}

pub fn identity_pk_to_external_identity(pk: SignaturePublicKey) -> Result<String, &'static str> {
    let pk_string = signature_public_key_to_string(pk);
    for (global_identity, data) in &*IDENTITY_MAP {
        if signature_public_key_to_string(data.signature_public_key) == pk_string {
            return Ok(global_identity.clone());
        }
    }
    Err("Unrecognized public key")
}

pub fn addr_to_external_profile_data(addr: SocketAddr) -> Vec<ExternalProfileData> {
    let mut result = Vec::new();

    for (global_identity, data) in &*IDENTITY_MAP {
        if data.addr == addr {
            result.push(ExternalProfileData {
                node_identity_name: data.node_identity_name.clone(),
                addr: data.addr,
                signature_public_key: data.signature_public_key.clone(),
                encryption_public_key: data.encryption_public_key.clone(),
            });
        }
    }

    result
}