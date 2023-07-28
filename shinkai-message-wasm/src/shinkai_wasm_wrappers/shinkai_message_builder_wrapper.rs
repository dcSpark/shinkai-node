use wasm_bindgen::prelude::*;
use crate::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};
use js_sys::Uint8Array;

#[wasm_bindgen]
pub struct ShinkaiMessageBuilderWrapper {
    inner: ShinkaiMessageBuilder,
}

#[wasm_bindgen]
impl ShinkaiMessageBuilderWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(
        my_encryption_secret_key: JsValue,
        my_signature_secret_key: JsValue,
        receiver_public_key: JsValue,
    ) -> Result<ShinkaiMessageBuilderWrapper, JsValue> {
        let my_encryption_secret_key = convert_jsvalue_to_encryptionstatickey(my_encryption_secret_key)?;
        let my_signature_secret_key = convert_jsvalue_to_signaturestatickey(my_signature_secret_key)?;
        let receiver_public_key = convert_jsvalue_to_encryptionpublickey(receiver_public_key)?;

        let inner = ShinkaiMessageBuilder::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
        );

        Ok(ShinkaiMessageBuilderWrapper { inner })
    }

    // #[wasm_bindgen]
    // pub fn body_encryption(&mut self, encryption: JsValue) -> Result<(), JsValue> {
    //     let encryption = convert_jsvalue_to_someencryption(encryption)?; // replace this with actual conversion function
    //     self.inner = self.inner.body_encryption(encryption);
    //     Ok(())
    // }

    // #[wasm_bindgen]
    // pub fn no_body_encryption(&mut self) -> Result<(), JsValue> {
    //     self.inner = self.inner.no_body_encryption();
    //     Ok(())
    // }

    // #[wasm_bindgen]
    // pub fn body(&mut self, content: String) -> Result<(), JsValue> {
    //     self.inner = self.inner.body(content);
    //     Ok(())
    // }

    // #[wasm_bindgen]
    // pub fn message_schema_type(&mut self, content: JsValue) -> Result<(), JsValue> {
    //     let content = convert_jsvalue_to_messageschematype(content)?; // replace this with actual conversion function
    //     self.inner = self.inner.message_schema_type(content);
    //     Ok(())
    // }

    // #[wasm_bindgen]
    // pub fn internal_metadata(
    //     &mut self,
    //     sender_subidentity: String,
    //     recipient_subidentity: String,
    //     inbox: String,
    //     encryption: JsValue,
    // ) -> Result<(), JsValue> {
    //     let encryption = convert_jsvalue_to_encryptionmethod(encryption)?; // replace this with actual conversion function
    //     self.inner = self.inner.internal_metadata(sender_subidentity, recipient_subidentity, inbox, encryption);
    //     Ok(())
    // }

    // Add more methods for ShinkaiMessageBuilderWrapper
    // The main idea is to convert JavaScript compatible inputs to Rust inputs,
    // call the corresponding method on `self.inner`, and then return a `Result<T, JsValue>`
}

fn convert_jsvalue_to_encryptionstatickey(val: JsValue) -> Result<EncryptionStaticKey, JsValue> {
    let arr: Uint8Array = val.dyn_into()?;
    let mut bytes = [0u8; 32];
    arr.copy_to(&mut bytes);
    Ok(EncryptionStaticKey::from(bytes))
}

fn convert_jsvalue_to_signaturestatickey(val: JsValue) -> Result<SignatureStaticKey, JsValue> {
    let arr: Uint8Array = val.dyn_into()?;
    let bytes: Vec<u8> = arr.to_vec();
    Ok(SignatureStaticKey::from_bytes(&bytes).map_err(|_| JsValue::from_str("Invalid signature key"))?)
}

fn convert_jsvalue_to_encryptionpublickey(val: JsValue) -> Result<EncryptionPublicKey, JsValue> {
    let arr: Uint8Array = val.dyn_into()?;
    let mut bytes = [0u8; 32];
    arr.copy_to(&mut bytes);
    Ok(EncryptionPublicKey::from(bytes))
}