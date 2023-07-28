

// #[wasm_bindgen]
// pub struct ShinkaiMessageBuilder {
//     body: Option<Body>,
//     message_schema_type: String,
//     internal_metadata: Option<InternalMetadata>,
//     external_metadata: Option<ExternalMetadata>,
//     encryption: String,
//     my_encryption_secret_key: EncryptionStaticKey,
//     my_encryption_public_key: EncryptionPublicKey,
//     my_signature_secret_key: SignatureStaticKey,
//     my_signature_public_key: SignaturePublicKey,
//     receiver_public_key: EncryptionPublicKey,
// }

// #[wasm_bindgen]
// impl ShinkaiMessageBuilder {
//     pub fn new(
//         my_encryption_secret_key: EncryptionStaticKey,
//         my_signature_secret_key: SignatureStaticKey,
//         receiver_public_key: EncryptionPublicKey,
//     ) -> Self {
//         let my_encryption_public_key = x25519_dalek::PublicKey::from(&my_encryption_secret_key);
//         let my_signature_public_key = ed25519_dalek::PublicKey::from(&my_signature_secret_key);
//         Self {
//             body: None,
//             message_schema_type: String::new(),
//             internal_metadata: None,
//             external_metadata: None,
//             encryption: EncryptionMethod::None.as_str().to_string(),
//             my_encryption_secret_key,
//             my_encryption_public_key,
//             my_signature_public_key,
//             my_signature_secret_key,
//             receiver_public_key,
//         }
//     }
// }