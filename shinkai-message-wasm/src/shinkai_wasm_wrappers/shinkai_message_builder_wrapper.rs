use crate::schemas::inbox_name::InboxName;
use crate::schemas::registration_code::RegistrationCode;
use crate::shinkai_message::shinkai_message_schemas::{JobCreation, JobMessage, JobScope, RegistrationCodeRequest, RegistrationCodeType, IdentityPermissions};
use crate::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, string_to_encryption_public_key,
    string_to_encryption_static_key,
};
use crate::shinkai_utils::signatures::{
    signature_public_key_to_string, signature_secret_key_to_string, string_to_signature_secret_key,
};
use crate::shinkai_wasm_wrappers::shinkai_message_wrapper::ShinkaiMessageWrapper;
use crate::{
    shinkai_message::shinkai_message_schemas::MessageSchemaType,
    shinkai_utils::{
        encryption::EncryptionMethod,
        shinkai_message_builder::{ProfileName, ShinkaiMessageBuilder},
    },
};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use js_sys::Uint8Array;
use serde::{Deserialize, Serialize};
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
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
    ) -> Result<ShinkaiMessageBuilderWrapper, JsValue> {
        let my_encryption_secret_key = string_to_encryption_static_key(&my_encryption_secret_key)?;
        let my_signature_secret_key = string_to_signature_secret_key(&my_signature_secret_key)?;
        let receiver_public_key = string_to_encryption_public_key(&receiver_public_key)?;

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
        encryption: JsValue,
    ) -> Result<(), JsValue> {
        let encryption = convert_jsvalue_to_encryptionmethod(encryption)?;

        if let Some(mut inner) = self.inner.take() {
            inner = inner.internal_metadata(sender_subidentity, recipient_subidentity, encryption);
            self.inner = Some(inner);
            Ok(())
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn internal_metadata_with_inbox(
        &mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        encryption: JsValue,
    ) -> Result<(), JsValue> {
        let encryption = convert_jsvalue_to_encryptionmethod(encryption)?;

        if let Some(mut inner) = self.inner.take() {
            inner = inner.internal_metadata_with_inbox(sender_subidentity, recipient_subidentity, inbox, encryption);
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

    #[wasm_bindgen]
    pub fn build_to_jsvalue(&mut self) -> Result<JsValue, JsValue> {
        if let Some(ref builder) = self.inner {
            match builder.build() {
                Ok(shinkai_message) => {
                    let js_value = shinkai_message.to_jsvalue()?;
                    Ok(js_value)
                }
                Err(e) => Err(JsValue::from_str(e)),
            }
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn build_to_string(&mut self) -> Result<String, JsValue> {
        if let Some(ref builder) = self.inner {
            match builder.build() {
                Ok(shinkai_message) => {
                    let json =
                        serde_json::to_string(&shinkai_message).map_err(|e| JsValue::from_str(&e.to_string()))?;
                    Ok(json)
                }
                Err(e) => Err(JsValue::from_str(e)),
            }
        } else {
            Err(JsValue::from_str(
                "Inner ShinkaiMessageBuilder is None. This should never happen.",
            ))
        }
    }

    #[wasm_bindgen]
    pub fn ack_message(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<String, JsValue> {
        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        let _ = builder.body("ACK".to_string());
        let _ = builder.empty_non_encrypted_internal_metadata();
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata(receiver, sender);
        builder.build_to_string()
    }

    #[wasm_bindgen]
    pub fn request_code_registration(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        permissions: String,
        code_type: String,
        sender_profile_name: String,
        receiver: ProfileName,
    ) -> Result<String, JsValue> {
        let permissions =
            IdentityPermissions::from_str(&permissions).ok_or_else(|| JsValue::from_str("Invalid permissions"))?;
        let code_type = RegistrationCodeType::deserialize(serde_json::Value::String(code_type))
            .map_err(|_| JsValue::from_str("Invalid code type"))?;
        let registration_code_request = RegistrationCodeRequest { permissions, code_type };
        let data = registration_code_request.to_json_str()?;

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            data,
            sender_profile_name,
            receiver,
            MessageSchemaType::CreateRegistrationCode.to_str().to_string(),
        )
    }

    #[wasm_bindgen]
    pub fn use_code_registration(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        code: String,
        identity_type: String,
        permission_type: String,
        registration_name: String,
        sender_profile_name: String,
        receiver: ProfileName,
    ) -> Result<String, JsValue> {
        let my_subidentity_encryption_sk = string_to_encryption_static_key(&my_subidentity_encryption_sk)?;
        let my_subidentity_signature_sk = string_to_signature_secret_key(&my_subidentity_signature_sk)?;
        let receiver_public_key = string_to_encryption_public_key(&receiver_public_key)?;

        let my_subidentity_signature_pk = ed25519_dalek::PublicKey::from(&my_subidentity_signature_sk);
        let my_subidentity_encryption_pk = x25519_dalek::PublicKey::from(&my_subidentity_encryption_sk);

        let other = encryption_public_key_to_string(my_subidentity_encryption_pk);
        let registration_code = RegistrationCode {
            code,
            registration_name: registration_name.clone(),
            identity_pk: signature_public_key_to_string(my_subidentity_signature_pk),
            encryption_pk: other.clone(),
            identity_type,
            permission_type,
        };

        let body = serde_json::to_string(&registration_code).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let my_subidentity_encryption_sk_str = encryption_secret_key_to_string(my_subidentity_encryption_sk);
        let my_subidentity_signature_sk_str = signature_secret_key_to_string(my_subidentity_signature_sk);
        let receiver_public_key_str = encryption_public_key_to_string(receiver_public_key);

        let mut builder = ShinkaiMessageBuilderWrapper::new(
            my_subidentity_encryption_sk_str,
            my_subidentity_signature_sk_str,
            receiver_public_key_str,
        )?;
        let body_encryption = JsValue::from_str(EncryptionMethod::DiffieHellmanChaChaPoly1305.as_str());
        let internal_encryption = JsValue::from_str(EncryptionMethod::None.as_str());
        let _ = builder.body(body);
        let _ = builder.body_encryption(body_encryption);
        let _ = builder.internal_metadata(sender_profile_name, "".to_string(), internal_encryption);
        let _ = builder.external_metadata_with_other(receiver.clone(), receiver, other);

        builder.build_to_string()
    }

    pub fn create_custom_shinkai_message_to_node(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        data: String,
        sender_profile_name: String,
        receiver: ProfileName,
        schema: String,
    ) -> Result<String, JsValue> {
        let my_subidentity_encryption_sk_type = string_to_encryption_static_key(&my_subidentity_encryption_sk)?;
        let my_subidentity_encryption_pk = x25519_dalek::PublicKey::from(&my_subidentity_encryption_sk_type);
        let other = encryption_public_key_to_string(my_subidentity_encryption_pk);

        let mut builder = ShinkaiMessageBuilderWrapper::new(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
        )?;
        let body_encryption = JsValue::from_str(EncryptionMethod::DiffieHellmanChaChaPoly1305.as_str());
        let internal_encryption = JsValue::from_str(EncryptionMethod::None.as_str());
        let schema_jsvalue = JsValue::from_str(&schema);

        let _ = builder.body(data);
        let _ = builder.body_encryption(body_encryption);
        let _ = builder.external_metadata_with_other(receiver.clone(), receiver, other);
        let _ = builder.internal_metadata_with_schema(
            sender_profile_name,
            "".to_string(),
            "".to_string(),
            schema_jsvalue,
            internal_encryption,
        );
        builder.build_to_string()
    }

    #[wasm_bindgen]
    pub fn ping_pong_message(
        message: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<String, JsValue> {
        if message != "Ping" && message != "Pong" {
            return Err(JsValue::from_str("Invalid message: must be 'Ping' or 'Pong'"));
        }

        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        let _ = builder.body(message);
        let _ = builder.empty_non_encrypted_internal_metadata();
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata(receiver, sender);

        builder.build_to_string()
    }

    #[wasm_bindgen]
    pub fn job_creation(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        scope: JsValue,
        sender: ProfileName,
        receiver: ProfileName,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let scope: JobScope = serde_wasm_bindgen::from_value(scope).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let job_creation = JobCreation { scope };
        let body = serde_json::to_string(&job_creation).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        let _ = builder.body(body);
        let _ = builder.internal_metadata_with_schema(
            "".to_string(),
            receiver_subidentity.clone(),
            "".to_string(),
            JsValue::from_str("JobCreationSchema"),
            JsValue::from_str("None"),
        );
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata(receiver, sender);

        builder.build_to_string()
    }

    #[wasm_bindgen]
    pub fn job_message(
        job_id: String,
        content: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ProfileName,
        receiver: ProfileName,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let job_id_clone = job_id.clone();
        let job_message = JobMessage { job_id, content };

        let body = serde_json::to_string(&job_message).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        let inbox = InboxName::get_job_inbox_name_from_params(job_id_clone)
            .map_err(|e| JsValue::from_str(&e.to_string()))?
            .get_value();

        let _ = builder.body(body);
        let _ = builder.internal_metadata_with_schema(
            "".to_string(),
            receiver_subidentity.clone(),
            inbox,
            JsValue::from_str("JobMessageSchema"),
            JsValue::from_str("None"),
        );
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata(receiver, sender);

        builder.build_to_string()
    }

    #[wasm_bindgen]
    pub fn terminate_message(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<String, JsValue> {
        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        let _ = builder.body("terminate".to_string());
        let _ = builder.empty_non_encrypted_internal_metadata();
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata(receiver, sender);

        builder.build_to_string()
    }

    #[wasm_bindgen]
    pub fn error_message(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ProfileName,
        receiver: ProfileName,
        error_msg: String,
    ) -> Result<String, JsValue> {
        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        let _ = builder.body(format!("{{error: \"{}\"}}", error_msg))?;
        let _ = builder.empty_encrypted_internal_metadata();
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata(receiver, sender);
        builder.build_to_string()
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
