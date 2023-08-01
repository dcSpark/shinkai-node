// use wasm_bindgen::JsValue;
// use wasm_bindgen::prelude::wasm_bindgen;
// use x25519_dalek::{PublicKey, StaticSecret};

// use crate::shinkai_utils::encryption::{encryption_secret_key_to_string, encryption_public_key_to_string};

// #[wasm_bindgen]
// pub struct KeyPair {
//     public_key: String,
//     private_key: String,
// }

// #[wasm_bindgen]
// impl KeyPair {
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
// pub fn wasm_ephemeral_encryption_keys() -> KeyPair {
//     #[allow(deprecated)]
//     let mut csprng = rand_os::OsRng::new().unwrap();
//     let secret_key = StaticSecret::new(&mut csprng);
//     let public_key = PublicKey::from(&secret_key);

//     // Convert keys to string
//     let secret_key_string = encryption_secret_key_to_string(secret_key);
//     let public_key_string = encryption_public_key_to_string(public_key);

//     KeyPair {
//         public_key: public_key_string,
//         private_key: secret_key_string,
//     }
// }
