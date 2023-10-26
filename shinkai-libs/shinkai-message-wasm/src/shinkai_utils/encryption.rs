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
use shinkai_message_primitives::shinkai_utils::encryption::{string_to_encryption_static_key, encryption_public_key_to_string, EncryptionMethod};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;
use x25519_dalek::{PublicKey, StaticSecret};

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WasmEncryptionMethod {
    method: EncryptionMethod,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WasmEncryptionMethod {
    #[wasm_bindgen(constructor)]
    pub fn new(method: &str) -> WasmEncryptionMethod {
        WasmEncryptionMethod {
            method: EncryptionMethod::from_str(method),
        }
    }

    #[wasm_bindgen]
    pub fn as_str(&self) -> String {
        self.method.as_str().to_string()
    }

    #[wasm_bindgen(js_name = "DiffieHellmanChaChaPoly1305")]
    pub fn diffie_hellman_cha_cha_poly1305() -> String {
        EncryptionMethod::DiffieHellmanChaChaPoly1305.as_str().to_string()
    }

    #[wasm_bindgen(js_name = "None")]
    pub fn none() -> String {
        EncryptionMethod::None.as_str().to_string()
    }
}

#[wasm_bindgen]
pub fn convert_encryption_sk_string_to_encryption_pk_string(encryption_sk: String) -> Result<String, JsValue> {
    let my_encryption_sk_type = string_to_encryption_static_key(&encryption_sk)?;
    let my_encryption_pk = x25519_dalek::PublicKey::from(&my_encryption_sk_type);
    Ok(encryption_public_key_to_string(my_encryption_pk))
}