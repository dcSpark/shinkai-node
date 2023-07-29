use crate::{
    shinkai_message::shinkai_message_schemas::MessageSchemaType,
    shinkai_utils::{
        encryption::EncryptionMethod,
        shinkai_message_builder::{ProfileName, ShinkaiMessageBuilder},
    }
};
use crate::shinkai_wasm_wrappers::shinkai_message_wrapper::ShinkaiMessageWrapper;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[wasm_bindgen]
pub struct ShinkaiMessageBuilderWrapper {
    inner: Option<ShinkaiMessageBuilder>,
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

        let inner = ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key);

        Ok(ShinkaiMessageBuilderWrapper { inner: Some(inner) })
    }

    #[wasm_bindgen]
    pub fn body_encryption(&mut self, encryption: JsValue) -> Result<(), JsValue> {
        let encryption = convert_jsvalue_to_encryptionmethod(encryption)?;

        if let Some(mut inner) = self.inner.take() {
            inner = inner.body_encryption(encryption);
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn no_body_encryption(&mut self) -> Result<(), JsValue> {
        if let Some(mut inner) = self.inner.take() {
            inner = inner.no_body_encryption();
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn body(&mut self, content: String) -> Result<(), JsValue> {
        if let Some(mut inner) = self.inner.take() {
            inner = inner.body(content);
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn message_schema_type(&mut self, content: JsValue) -> Result<(), JsValue> {
        let content = convert_jsvalue_to_messageschematype(content)?;

        if let Some(mut inner) = self.inner.take() {
            inner = inner.message_schema_type(content);
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn internal_metadata(
        &mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        encryption: JsValue,
    ) -> Result<(), JsValue> {
        let encryption = convert_jsvalue_to_encryptionmethod(encryption)?;

        if let Some(mut inner) = self.inner.take() {
            inner = inner.internal_metadata(sender_subidentity, recipient_subidentity, inbox, encryption);
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn internal_metadata_with_schema(
        &mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        message_schema: JsValue,
        encryption: JsValue,
    ) -> Result<(), JsValue> {
        let encryption = convert_jsvalue_to_encryptionmethod(encryption)?;
        let message_schema = convert_jsvalue_to_messageschematype(message_schema)?;

        if let Some(mut inner) = self.inner.take() {
            inner = inner.internal_metadata_with_schema(
                sender_subidentity,
                recipient_subidentity,
                inbox,
                message_schema,
                encryption,
            );
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn empty_encrypted_internal_metadata(&mut self) -> Result<(), JsValue> {
        if let Some(mut inner) = self.inner.take() {
            inner = inner.empty_encrypted_internal_metadata();
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn empty_non_encrypted_internal_metadata(&mut self) -> Result<(), JsValue> {
        if let Some(mut inner) = self.inner.take() {
            inner = inner.empty_non_encrypted_internal_metadata();
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn external_metadata(&mut self, recipient: String, sender: String) -> Result<(), JsValue> {
        if let Some(mut inner) = self.inner.take() {
            inner = inner.external_metadata(recipient, sender);
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn external_metadata_with_other(
        &mut self,
        recipient: String,
        sender: String,
        other: String,
    ) -> Result<(), JsValue> {
        if let Some(mut inner) = self.inner.take() {
            inner = inner.external_metadata_with_other(recipient, sender, other);
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn external_metadata_with_schedule(
        &mut self,
        recipient: String,
        sender: String,
        scheduled_time: String,
    ) -> Result<(), JsValue> {
        if let Some(mut inner) = self.inner.take() {
            inner = inner.external_metadata_with_schedule(
                ProfileName::from(recipient),
                ProfileName::from(sender),
                scheduled_time,
            );
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn build(&mut self) -> Result<ShinkaiMessageWrapper, JsValue> {
        if let Some(ref builder) = self.inner {
            match builder.build() {
                Ok(shinkai_message) => {
                    let js_value = shinkai_message.to_jsvalue()?;
                    Ok(ShinkaiMessageWrapper::from_jsvalue(&js_value)?)
                }
                Err(e) => Err(JsValue::from_str(e)),
            }
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }
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

fn convert_jsvalue_to_encryptionmethod(val: JsValue) -> Result<EncryptionMethod, JsValue> {
    let s = val
        .as_string()
        .ok_or_else(|| JsValue::from_str("Expected string for EncryptionMethod"))?;
    Ok(EncryptionMethod::from_str(&s))
}

fn convert_jsvalue_to_messageschematype(val: JsValue) -> Result<MessageSchemaType, JsValue> {
    let s = val
        .as_string()
        .ok_or_else(|| JsValue::from_str("Expected string for MessageSchemaType"))?;
    MessageSchemaType::from_str(&s).ok_or_else(|| JsValue::from_str("Invalid MessageSchemaType"))
}
