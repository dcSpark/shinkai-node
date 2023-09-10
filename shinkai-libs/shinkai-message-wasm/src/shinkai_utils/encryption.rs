use core::fmt;
use std::convert::TryInto;
use std::error::Error;
use chacha20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use chacha20poly1305::ChaCha20Poly1305;
use js_sys::Uint8Array;
// Or use ChaCha20Poly1305Ietf
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shinkai_message_primitives::shinkai_utils::encryption::{string_to_encryption_static_key, encryption_public_key_to_string};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;
use x25519_dalek::{PublicKey, StaticSecret};
use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;

// #[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
// #[wasm_bindgen]
// pub enum EncryptionMethod {
//     DiffieHellmanChaChaPoly1305,
//     None,
// }

// impl EncryptionMethod {
//     pub fn as_str(&self) -> &'static str {
//         match self {
//             Self::DiffieHellmanChaChaPoly1305 => "default",
//             Self::None => "None",
//         }
//     }

//     pub fn from_str(s: &str) -> EncryptionMethod {
//         match s {
//             "DiffieHellmanChaChaPoly1305" | "default" => EncryptionMethod::DiffieHellmanChaChaPoly1305,
//             _ => EncryptionMethod::None,
//         }
//     }
// }

#[wasm_bindgen]
pub fn convert_encryption_sk_string_to_encryption_pk_string(encryption_sk: String) -> Result<String, JsValue> {
    let my_encryption_sk_type = string_to_encryption_static_key(&encryption_sk)?;
    let my_encryption_pk = x25519_dalek::PublicKey::from(&my_encryption_sk_type);
    Ok(encryption_public_key_to_string(my_encryption_pk))
}