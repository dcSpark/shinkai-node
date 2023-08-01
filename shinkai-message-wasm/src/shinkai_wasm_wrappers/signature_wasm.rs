// use wasm_bindgen::JsValue;
// use wasm_bindgen::prelude::wasm_bindgen;
// use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer, Verifier};

// use crate::shinkai_utils::{encryption::{encryption_secret_key_to_string, encryption_public_key_to_string}, signatures::{signature_secret_key_to_string, signature_public_key_to_string}};

// #[wasm_bindgen]
// pub struct SignatureKeyPair {
//     public_key: String,
//     private_key: String,
// }

// #[wasm_bindgen]
// impl SignatureKeyPair {
//     #[wasm_bindgen(getter)]
//     pub fn public_key(&self) -> String {
//         self.public_key.clone()
//     }

//     #[wasm_bindgen(getter)]
//     pub fn private_key(&self) -> String {
//         self.private_key.clone()
//     }
// }

// #[wasm_bindgen]
// pub fn wasm_ephemeral_signature_keypair() -> SignatureKeyPair {
//     #[warn(deprecated)]
//     let mut csprng = rand_os::OsRng::new().unwrap();
//     let keypair = Keypair::generate(&mut csprng);

//     // Convert keys to string
//     let secret_key_string = signature_secret_key_to_string(keypair.secret);
//     let public_key_string = signature_public_key_to_string(keypair.public);

//     SignatureKeyPair {
//         public_key: public_key_string,
//         private_key: secret_key_string,
//     }
// }
