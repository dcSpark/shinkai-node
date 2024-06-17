use crate::shinkai_wasm_wrappers::{
    shinkai_message_wrapper::ShinkaiMessageWrapper,
    shinkai_wasm_error::{ShinkaiWasmError, WasmErrorWrapper},
    wasm_shinkai_message::SerdeWasmMethods,
};
use serde::Deserialize;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::{
    schemas::{llm_providers::serialized_llm_provider::SerializedLLMProvider, inbox_name::InboxName, registration_code::RegistrationCode},
    shinkai_message::shinkai_message_schemas::{
        APIAddAgentRequest, APIGetMessagesFromInboxRequest, APIReadUpToTimeRequest, IdentityPermissions,
        JobCreationInfo, JobMessage, MessageSchemaType, RegistrationCodeRequest, RegistrationCodeType,
    },
    shinkai_utils::{
        encryption::{
            encryption_public_key_to_string, string_to_encryption_public_key, string_to_encryption_static_key,
            EncryptionMethod,
        },
        shinkai_message_builder::{ShinkaiMessageBuilder, ShinkaiNameString},
        signatures::{signature_public_key_to_string, string_to_signature_secret_key},
    },
};
use wasm_bindgen::prelude::*;

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
    pub fn message_raw_content(&mut self, message_raw_content: String) -> Result<(), JsValue> {
        if let Some(mut inner) = self.inner.take() {
            inner = inner.message_raw_content(message_raw_content);
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
        sender_subidentity: ShinkaiNameString,
        recipient_subidentity: String,
        encryption: JsValue,
    ) -> Result<(), JsValue> {
        let encryption = convert_jsvalue_to_encryptionmethod(encryption)?;

        if let Some(mut inner) = self.inner.take() {
            inner = inner.internal_metadata(sender_subidentity, recipient_subidentity, encryption, None);
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
        sender_subidentity: ShinkaiNameString,
        recipient_subidentity: String,
        inbox: String,
        encryption: JsValue,
    ) -> Result<(), JsValue> {
        let encryption = convert_jsvalue_to_encryptionmethod(encryption)?;

        if let Some(mut inner) = self.inner.take() {
            inner =
                inner.internal_metadata_with_inbox(sender_subidentity, recipient_subidentity, inbox, encryption, None);
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
        sender_subidentity: ShinkaiNameString,
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
                None,
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
    pub fn external_metadata_with_intra(
        &mut self,
        recipient: String,
        sender: String,
        intra_sender: String,
    ) -> Result<(), JsValue> {
        if let Some(mut inner) = self.inner.take() {
            inner = inner.external_metadata_with_intra_sender(recipient, sender, intra_sender);
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
    pub fn external_metadata_with_other_and_intra_sender(
        &mut self,
        recipient: String,
        sender: String,
        other: String,
        intra_sender: String,
    ) -> Result<(), JsValue> {
        if let Some(mut inner) = self.inner.take() {
            inner = inner.external_metadata_with_other_and_intra_sender(recipient, sender, other, intra_sender);
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
                ShinkaiNameString::from(recipient),
                ShinkaiNameString::from(sender),
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
            // Print the hash to console if the target architecture is wasm
            // if cfg!(target_arch = "wasm32") {
            //     let body = format!("{:?}", builder);
            //     web_sys::console::log_1(&JsValue::from_str(&body));
            // }
            match builder.build() {
                Ok(shinkai_message) => {
                    let js_value = shinkai_message.to_jsvalue().map_err(WasmErrorWrapper)?;
                    Ok(ShinkaiMessageWrapper::from_jsvalue(&js_value)
                        .map_err(|e| WasmErrorWrapper::new(ShinkaiWasmError::from(e)))?)
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
            // Print the hash to console if the target architecture is wasm
            // if cfg!(target_arch = "wasm32") {
            //     let body = format!("{:?}", builder);
            //     web_sys::console::log_1(&JsValue::from_str(&body));
            // }
            match builder.build() {
                Ok(shinkai_message) => shinkai_message
                    .to_jsvalue()
                    .map_err(|e| JsValue::from_str(&e.to_string())),
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
            // Print the hash to console if the target architecture is wasm
            // if cfg!(target_arch = "wasm32") {
            //     let body = format!("{:?}", builder);
            //     web_sys::console::log_1(&JsValue::from_str(&body));
            // }
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
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
    ) -> Result<String, JsValue> {
        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        let _ = builder.message_raw_content("ACK".to_string());
        let _ = builder.empty_non_encrypted_internal_metadata();
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata_with_intra(receiver, sender, sender_subidentity);
        builder.build_to_string()
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn request_code_registration(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        permissions: String,
        code_type: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        recipient: ShinkaiNameString,
        recipient_subidentity: String,
    ) -> Result<String, JsValue> {
        let permissions =
            IdentityPermissions::from_str(&permissions).ok_or_else(|| JsValue::from_str("Invalid permissions"))?;
        let code_type = RegistrationCodeType::deserialize(serde_json::Value::String(code_type))
            .map_err(|_| JsValue::from_str("Invalid code type"))?;
        let registration_code_request = RegistrationCodeRequest { permissions, code_type };
        let data = match registration_code_request.to_json_str() {
            Ok(data) => data,
            Err(e) => return Err(JsValue::from_str(&e.to_string())),
        };

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            data,
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            "",
            MessageSchemaType::CreateRegistrationCode.to_str().to_string(),
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn use_code_registration_for_profile(
        profile_encryption_sk: String,
        profile_signature_sk: String,
        receiver_public_key: String,
        code: String,
        identity_type: String,
        permission_type: String,
        registration_name: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        recipient: ShinkaiNameString,
        recipient_subidentity: String,
    ) -> Result<String, JsValue> {
        let profile_encryption_sk_type = string_to_encryption_static_key(&profile_encryption_sk)?;
        let profile_signature_sk_type = string_to_signature_secret_key(&profile_signature_sk)?;

        let profile_signature_pk = profile_signature_sk_type.verifying_key();
        let profile_encryption_pk = x25519_dalek::PublicKey::from(&profile_encryption_sk_type);

        let registration_code = RegistrationCode {
            code,
            registration_name: registration_name.clone(),
            device_identity_pk: "".to_string(),
            device_encryption_pk: "".to_string(),
            profile_identity_pk: signature_public_key_to_string(profile_signature_pk),
            profile_encryption_pk: encryption_public_key_to_string(profile_encryption_pk),
            identity_type,
            permission_type,
        };

        let body = serde_json::to_string(&registration_code).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let other = encryption_public_key_to_string(profile_encryption_pk);

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            profile_encryption_sk,
            profile_signature_sk,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            other.as_str(),
            MessageSchemaType::TextContent.to_str().to_string(),
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn use_code_registration_for_device(
        my_device_encryption_sk: String,
        my_device_signature_sk: String,
        profile_encryption_sk: String,
        profile_signature_sk: String,
        receiver_public_key: String,
        code: String,
        identity_type: String,
        permission_type: String,
        registration_name: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        recipient: ShinkaiNameString,
        recipient_subidentity: String,
    ) -> Result<String, JsValue> {
        let my_subidentity_encryption_sk_type = string_to_encryption_static_key(&my_device_encryption_sk)?;
        let my_subidentity_signature_sk_type = string_to_signature_secret_key(&my_device_signature_sk)?;
        let profile_encryption_sk_type = string_to_encryption_static_key(&profile_encryption_sk)?;
        let profile_signature_sk_type = string_to_signature_secret_key(&profile_signature_sk)?;

        let my_subidentity_signature_pk = my_subidentity_signature_sk_type.verifying_key();
        let my_subidentity_encryption_pk = x25519_dalek::PublicKey::from(&my_subidentity_encryption_sk_type);
        let profile_signature_pk = profile_signature_sk_type.verifying_key();
        let profile_encryption_pk = x25519_dalek::PublicKey::from(&profile_encryption_sk_type);

        let other = encryption_public_key_to_string(my_subidentity_encryption_pk);
        let registration_code = RegistrationCode {
            code,
            registration_name: registration_name.clone(),
            device_identity_pk: signature_public_key_to_string(my_subidentity_signature_pk),
            device_encryption_pk: other.clone(),
            profile_identity_pk: signature_public_key_to_string(profile_signature_pk),
            profile_encryption_pk: encryption_public_key_to_string(profile_encryption_pk),
            identity_type,
            permission_type,
        };

        let body = serde_json::to_string(&registration_code).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let other = encryption_public_key_to_string(my_subidentity_encryption_pk);

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_device_encryption_sk,
            my_device_signature_sk,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            other.as_str(),
            MessageSchemaType::TextContent.to_str().to_string(),
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn initial_registration_with_no_code_for_device(
        my_device_encryption_sk: String,
        my_device_signature_sk: String,
        profile_encryption_sk: String,
        profile_signature_sk: String,
        registration_name: String,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
    ) -> Result<String, JsValue> {
        let my_device_encryption_sk_type = string_to_encryption_static_key(&my_device_encryption_sk)?;
        let my_device_signature_sk_type = string_to_signature_secret_key(&my_device_signature_sk)?;
        let profile_encryption_sk_type = string_to_encryption_static_key(&profile_encryption_sk)?;
        let profile_signature_sk_type = string_to_signature_secret_key(&profile_signature_sk)?;

        let my_device_signature_pk = my_device_signature_sk_type.verifying_key();
        let my_device_encryption_pk = x25519_dalek::PublicKey::from(&my_device_encryption_sk_type);
        let profile_signature_pk = profile_signature_sk_type.verifying_key();
        let profile_encryption_pk = x25519_dalek::PublicKey::from(&profile_encryption_sk_type);

        let other = encryption_public_key_to_string(my_device_encryption_pk);

        let identity_type = "device".to_string();
        let permission_type = "admin".to_string();

        let registration_code = RegistrationCode {
            code: "".to_string(),
            registration_name: registration_name.clone(),
            device_identity_pk: signature_public_key_to_string(my_device_signature_pk),
            device_encryption_pk: other.clone(),
            profile_identity_pk: signature_public_key_to_string(profile_signature_pk),
            profile_encryption_pk: encryption_public_key_to_string(profile_encryption_pk),
            identity_type,
            permission_type,
        };

        let body = serde_json::to_string(&registration_code).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let other = encryption_public_key_to_string(my_device_encryption_pk);
        let schema_jsvalue = JsValue::from_str(MessageSchemaType::TextContent.to_str());
        let internal_encryption = JsValue::from_str(EncryptionMethod::None.as_str());

        let mut builder = ShinkaiMessageBuilderWrapper::new(
            my_device_encryption_sk,
            my_device_signature_sk,
            encryption_public_key_to_string(my_device_encryption_pk),
        )?;

        let _ = builder.message_raw_content(body);
        let _ = builder.no_body_encryption();
        let _ = builder.empty_non_encrypted_internal_metadata();
        let _ = builder.external_metadata_with_other(receiver, sender, other.to_string());
        let _ = builder.internal_metadata_with_schema(
            sender_subidentity,
            "".to_string(),
            "".to_string(),
            schema_jsvalue,
            internal_encryption,
        );
        builder.build_to_string()
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn get_last_messages_from_inbox(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        inbox: String,
        count: usize,
        offset: Option<String>,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        recipient: ShinkaiNameString,
        recipient_subidentity: String,
    ) -> Result<String, JsValue> {
        let inbox_name = InboxName::new(inbox.clone()).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let get_last_messages_from_inbox = APIGetMessagesFromInboxRequest {
            inbox: inbox_name.to_string(),
            count,
            offset,
        };

        let body =
            serde_json::to_string(&get_last_messages_from_inbox).map_err(|e| JsValue::from_str(&e.to_string()))?;

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            "",
            MessageSchemaType::APIGetMessagesFromInboxRequest.to_str().to_string(),
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn get_last_unread_messages_from_inbox(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        inbox: String,
        count: usize,
        offset: Option<String>,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        recipient: ShinkaiNameString,
        recipient_subidentity: String,
    ) -> Result<String, JsValue> {
        let inbox_name = InboxName::new(inbox.clone()).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let get_last_unread_messages_from_inbox = APIGetMessagesFromInboxRequest {
            inbox: inbox_name.to_string(),
            count,
            offset,
        };

        let body = serde_json::to_string(&get_last_unread_messages_from_inbox)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            "",
            MessageSchemaType::APIGetMessagesFromInboxRequest.to_str().to_string(),
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn request_add_agent(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        agent_json: &str,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        recipient: ShinkaiNameString,
        recipient_subidentity: String,
    ) -> Result<String, JsValue> {
        let agent: SerializedLLMProvider =
            serde_json::from_str(agent_json).map_err(|_| "Failed to deserialize agent from JSON")?;

        let add_agent_request = APIAddAgentRequest { agent };
        let body = serde_json::to_string(&add_agent_request).map_err(|e| JsValue::from_str(&e.to_string()))?;

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            "",
            MessageSchemaType::APIAddAgentRequest.to_str().to_string(),
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn get_all_availability_agent(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        recipient: ShinkaiNameString,
        recipient_subidentity: String,
    ) -> Result<String, JsValue> {
        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            "".to_string(),
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            "",
            MessageSchemaType::Empty.to_str().to_string(),
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn read_up_to_time(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        inbox: String,
        up_to_time: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        recipient: ShinkaiNameString,
        recipient_subidentity: String,
    ) -> Result<String, JsValue> {
        let inbox_name = InboxName::new(inbox.clone()).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let read_up_to_time = APIReadUpToTimeRequest { inbox_name, up_to_time };

        let body = serde_json::to_string(&read_up_to_time).map_err(|e| JsValue::from_str(&e.to_string()))?;

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            "",
            MessageSchemaType::APIReadUpToTimeRequest.to_str().to_string(),
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn create_custom_shinkai_message_to_node(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        data: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        recipient: ShinkaiNameString,
        recipient_subidentity: String,
        other: &str,
        schema: String,
    ) -> Result<String, JsValue> {
        let mut builder = ShinkaiMessageBuilderWrapper::new(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
        )?;
        let body_encryption = JsValue::from_str(EncryptionMethod::DiffieHellmanChaChaPoly1305.as_str());
        let internal_encryption = JsValue::from_str(EncryptionMethod::None.as_str());
        let schema_jsvalue = JsValue::from_str(&schema);

        let _ = builder.message_raw_content(data);
        let _ = builder.body_encryption(body_encryption);
        let _ = builder.external_metadata_with_other_and_intra_sender(
            recipient,
            sender,
            other.to_string(),
            sender_subidentity.clone(),
        );
        let _ = builder.internal_metadata_with_schema(
            sender_subidentity,
            recipient_subidentity,
            "".to_string(),
            schema_jsvalue,
            internal_encryption,
        );
        builder.build_to_string()
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn ping_pong_message(
        message: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
    ) -> Result<String, JsValue> {
        if message != "Ping" && message != "Pong" {
            return Err(JsValue::from_str("Invalid message: must be 'Ping' or 'Pong'"));
        }

        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        let _ = builder.message_raw_content(message);
        let _ = builder.empty_non_encrypted_internal_metadata();
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata(receiver, sender);

        builder.build_to_string()
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn job_creation(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        scope: JsValue,
        is_hidden: bool,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let scope: JobScope = serde_wasm_bindgen::from_value(scope).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let job_creation = JobCreationInfo {
            scope,
            is_hidden: Some(is_hidden),
        };
        let body = serde_json::to_string(&job_creation).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        let _ = builder.message_raw_content(body);
        let _ = builder.internal_metadata_with_schema(
            sender_subidentity.clone().to_string(),
            receiver_subidentity.clone(),
            "".to_string(),
            JsValue::from_str("JobCreationSchema"),
            JsValue::from_str("None"),
        );
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata_with_intra(receiver, sender, sender_subidentity);

        builder.build_to_string()
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn job_message(
        job_id: String,
        content: String,
        files_inbox: String,
        parent: String,
        workflow: Option<String>,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let job_id_clone = job_id.clone();
        let job_message = JobMessage {
            job_id,
            content,
            files_inbox,
            parent: Some(parent),
            workflow,
        };

        let body = serde_json::to_string(&job_message).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        let inbox = InboxName::get_job_inbox_name_from_params(job_id_clone)
            .map_err(|e| JsValue::from_str(&e.to_string()))?
            .to_string();

        let _ = builder.message_raw_content(body);
        let _ = builder.internal_metadata_with_schema(
            sender_subidentity.clone().to_string(),
            receiver_subidentity.clone(),
            inbox,
            JsValue::from_str("JobMessageSchema"),
            JsValue::from_str("None"),
        );
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata_with_intra(receiver, sender, sender_subidentity);

        builder.build_to_string()
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn terminate_message(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
    ) -> Result<String, JsValue> {
        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        let _ = builder.message_raw_content("terminate".to_string());
        let _ = builder.empty_non_encrypted_internal_metadata();
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata_with_intra(receiver, sender, sender_subidentity);

        builder.build_to_string()
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn error_message(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        error_msg: String,
    ) -> Result<String, JsValue> {
        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        builder.message_raw_content(format!("{{error: \"{}\"}}", error_msg))?;
        let _ = builder.empty_non_encrypted_internal_metadata();
        let _ = builder.body_encryption(JsValue::from_str(
            EncryptionMethod::DiffieHellmanChaChaPoly1305.as_str(),
        ));
        let _ = builder.external_metadata_with_intra(receiver, sender, sender_subidentity);
        builder.build_to_string()
    }
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
