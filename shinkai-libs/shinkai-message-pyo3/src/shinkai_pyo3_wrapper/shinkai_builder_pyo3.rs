use super::{
    encryption_method_pyo3::PyEncryptionMethod, message_schema_type_pyo3::PyMessageSchemaType,
    shinkai_message_pyo3::PyShinkaiMessage, shinkai_schema_pyo3::PyJobScope,
};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use pyo3::{prelude::*, pyclass, types::PyDict, PyResult};
use shinkai_message_primitives::{
    schemas::{agents::serialized_agent::SerializedAgent, inbox_name::InboxName, registration_code::RegistrationCode},
    shinkai_message::shinkai_message_schemas::{
        APIAddAgentRequest, APIGetMessagesFromInboxRequest, APIReadUpToTimeRequest, IdentityPermissions, JobCreationInfo, MessageSchemaType, RegistrationCodeRequest, RegistrationCodeType, JobMessage,
    },
    shinkai_utils::{
        encryption::{
            encryption_public_key_to_string, string_to_encryption_public_key, string_to_encryption_static_key,
            EncryptionMethod,
        },
        shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::{signature_public_key_to_string, string_to_signature_secret_key},
    },
};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[pyclass]
pub struct PyShinkaiMessageBuilder {
    pub inner: Option<ShinkaiMessageBuilder>,
}

#[pymethods]
impl PyShinkaiMessageBuilder {
    #[new]
    #[pyo3(text_signature = "(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)")]
    fn new(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
    ) -> PyResult<Self> {
        let my_encryption_secret_key = string_to_encryption_static_key(&my_encryption_secret_key)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))?;
        let my_signature_secret_key = string_to_signature_secret_key(&my_signature_secret_key)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))?;
        let receiver_public_key = string_to_encryption_public_key(&receiver_public_key)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))?;

        let inner = ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key);

        Ok(Self { inner: Some(inner) })
    }

    fn body_encryption(&mut self, encryption: Py<PyEncryptionMethod>) -> PyResult<()> {
        Python::with_gil(|py| {
            let encryption_ref = encryption.as_ref(py).borrow();
            if let Some(mut inner) = self.inner.take() {
                let new_inner = inner.body_encryption(encryption_ref.inner.clone());
                self.inner = Some(new_inner);
                Ok(())
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
            }
        })
    }

    fn no_body_encryption(&mut self) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.no_body_encryption();
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    fn message_raw_content(&mut self, message_raw_content: String) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.message_raw_content(message_raw_content);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    fn message_schema_type(&mut self, content: Py<PyMessageSchemaType>) -> PyResult<()> {
        Python::with_gil(|py| {
            let content_ref = content.as_ref(py).borrow();
            let rust_content = content_ref.inner.clone();
            if let Some(inner) = self.inner.take() {
                let mut inner_clone = inner.clone();
                inner_clone.message_schema_type(rust_content);
                self.inner = Some(inner);
                Ok(())
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
            }
        })
    }

    fn internal_metadata(
        &mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        encryption: Py<PyEncryptionMethod>,
    ) -> PyResult<()> {
        Python::with_gil(|py| {
            let encryption_ref = encryption.as_ref(py).borrow();
            if let Some(inner) = self.inner.take() {
                let new_inner =
                    inner.internal_metadata(sender_subidentity, recipient_subidentity, encryption_ref.inner.clone());
                self.inner = Some(new_inner);
                Ok(())
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
            }
        })
    }

    fn internal_metadata_with_inbox(
        &mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        encryption: Py<PyEncryptionMethod>,
    ) -> PyResult<()> {
        Python::with_gil(|py| {
            let encryption_ref = encryption.as_ref(py).borrow();
            if let Some(inner) = self.inner.take() {
                let new_inner = inner.internal_metadata_with_inbox(
                    sender_subidentity,
                    recipient_subidentity,
                    inbox,
                    encryption_ref.inner.clone(),
                );
                self.inner = Some(new_inner);
                Ok(())
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
            }
        })
    }

    fn internal_metadata_with_schema(
        &mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        message_schema: Py<PyMessageSchemaType>,
        encryption: Py<PyEncryptionMethod>,
    ) -> PyResult<()> {
        Python::with_gil(|py| {
            let encryption_ref = encryption.as_ref(py).borrow();
            let message_schema_ref = message_schema.as_ref(py).borrow();
            if let Some(inner) = self.inner.take() {
                let new_inner = inner.internal_metadata_with_schema(
                    sender_subidentity,
                    recipient_subidentity,
                    inbox,
                    message_schema_ref.inner.clone(),
                    encryption_ref.inner.clone(),
                );
                self.inner = Some(new_inner);
                Ok(())
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
            }
        })
    }

    fn empty_encrypted_internal_metadata(&mut self) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.empty_encrypted_internal_metadata();
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    fn empty_non_encrypted_internal_metadata(&mut self) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.empty_non_encrypted_internal_metadata();
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    fn external_metadata(&mut self, recipient: String, sender: String) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.external_metadata(recipient, sender);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    fn external_metadata_with_intra_sender(&mut self, recipient: String, sender: String, intra_sender: String) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.external_metadata_with_intra_sender(recipient, sender, intra_sender);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    fn external_metadata_with_other(&mut self, recipient: String, sender: String, other: String) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.external_metadata_with_other(recipient, sender, other);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    fn external_metadata_with_other_and_intra_sender(&mut self, recipient: String, sender: String, other: String, intra_sender: String) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.external_metadata_with_other_and_intra_sender(recipient, sender, other, intra_sender);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    fn external_metadata_with_schedule(
        &mut self,
        recipient: String,
        sender: String,
        scheduled_time: String,
    ) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.external_metadata_with_schedule(recipient, sender, scheduled_time);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    fn build(&mut self) -> PyResult<PyShinkaiMessage> {
        if let Some(inner) = self.inner.take() {
            match inner.build() {
                Ok(shinkai_message) => {
                    self.inner = Some(inner);
                    Ok(PyShinkaiMessage { inner: shinkai_message })
                }
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())),
            }
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    fn build_to_string(&mut self) -> PyResult<String> {
        if let Some(inner) = self.inner.take() {
            match inner.build() {
                Ok(shinkai_message) => {
                    self.inner = Some(inner);
                    serde_json::to_string(&shinkai_message)
                        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
                }
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())),
            }
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    #[staticmethod]
    fn ack_message(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: String,
        receiver: String,
    ) -> PyResult<String> {
        let mut builder =
            PyShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        builder.message_raw_content("ACK".to_string())?;
        builder.empty_non_encrypted_internal_metadata()?;
        builder.no_body_encryption()?;
        builder.external_metadata(receiver, sender)?;
        builder.build_to_string()
    }

    #[staticmethod]
    fn create_custom_shinkai_message_to_node(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        data: String,
        sender: String,
        sender_subidentity: String,
        recipient: String,
        recipient_subidentity: String,
        other: String,
        schema: Py<PyMessageSchemaType>,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let builder_result = PyShinkaiMessageBuilder::new(
                my_subidentity_encryption_sk,
                my_subidentity_signature_sk,
                receiver_public_key,
            );

            match builder_result {
                Ok(mut builder) => {
                    let body_encryption = Py::new(
                        py,
                        PyEncryptionMethod {
                            inner: EncryptionMethod::DiffieHellmanChaChaPoly1305,
                        },
                    )?;
                    let internal_encryption = Py::new(
                        py,
                        PyEncryptionMethod {
                            inner: EncryptionMethod::None,
                        },
                    )?;

                    match builder.message_raw_content(data) {
                        Ok(_) => (),
                        Err(e) => return Err(e),
                    }

                    match builder.body_encryption(body_encryption) {
                        Ok(_) => (),
                        Err(e) => return Err(e),
                    }

                    match builder.external_metadata_with_other_and_intra_sender(recipient, sender, other, sender_subidentity.clone()) {
                        Ok(_) => (),
                        Err(e) => return Err(e),
                    }

                    match builder.internal_metadata_with_schema(
                        sender_subidentity,
                        recipient_subidentity,
                        "".to_string(),
                        schema,
                        internal_encryption,
                    ) {
                        Ok(_) => (),
                        Err(e) => return Err(e),
                    }

                    builder.build_to_string()
                }
                Err(e) => Err(e),
            }
        })
    }

    #[staticmethod]
    fn request_code_registration(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        permissions: String,
        code_type: String,
        sender_subidentity: String,
        sender: String,
        receiver: String,
    ) -> PyResult<String> {
        let permissions = IdentityPermissions::from_str(&permissions)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid permissions"))?;
        let code_type: RegistrationCodeType = serde_json::from_str(&code_type)
            .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid code type"))?;
        let registration_code_request = RegistrationCodeRequest { permissions, code_type };
        let data = serde_json::to_string(&registration_code_request)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone()))?;

        Python::with_gil(|py| {
            let schema = Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::CreateRegistrationCode,
                },
            )?;
            Self::create_custom_shinkai_message_to_node(
                my_subidentity_encryption_sk,
                my_subidentity_signature_sk,
                receiver_public_key,
                data,
                sender,
                sender_subidentity,
                receiver.clone(),
                receiver,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    fn use_code_registration_for_profile(
        profile_encryption_sk: String,
        profile_signature_sk: String,
        receiver_public_key: String,
        code: String,
        identity_type: String,
        permission_type: String,
        registration_name: String,
        sender: String,
        sender_subidentity: String,
        recipient: String,
        recipient_subidentity: String,
    ) -> PyResult<String> {
        let profile_encryption_sk_type = match string_to_encryption_static_key(&profile_encryption_sk) {
            Ok(key) => key,
            Err(_) => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "Invalid encryption key",
                ))
            }
        };
        let profile_signature_sk_type = match string_to_signature_secret_key(&profile_signature_sk) {
            Ok(key) => key,
            Err(_) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid signature key")),
        };

        let profile_signature_pk = ed25519_dalek::PublicKey::from(&profile_signature_sk_type);
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

        let body = serde_json::to_string(&registration_code)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone()))?;
        let other = encryption_public_key_to_string(profile_encryption_pk);

        Python::with_gil(|py| {
            let schema = Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::TextContent,
                },
            )?;
            Self::create_custom_shinkai_message_to_node(
                profile_encryption_sk,
                profile_signature_sk,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                recipient.clone(),
                recipient,
                other,
                schema,
            )
        })
    }

    #[staticmethod]
    fn use_code_registration_for_device(
        my_device_encryption_sk: String,
        my_device_signature_sk: String,
        profile_encryption_sk: String,
        profile_signature_sk: String,
        receiver_public_key: String,
        code: String,
        identity_type: String,
        permission_type: String,
        registration_name: String,
        sender: String,
        sender_subidentity: String,
        recipient: String,
        recipient_subidentity: String,
    ) -> PyResult<String> {
        let my_subidentity_encryption_sk_type = match string_to_encryption_static_key(&my_device_encryption_sk) {
            Ok(key) => key,
            Err(_) => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "Invalid device encryption key",
                ))
            }
        };
        let my_subidentity_signature_sk_type = match string_to_signature_secret_key(&my_device_signature_sk) {
            Ok(key) => key,
            Err(_) => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "Invalid device signature key",
                ))
            }
        };
        let profile_encryption_sk_type = match string_to_encryption_static_key(&profile_encryption_sk) {
            Ok(key) => key,
            Err(_) => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "Invalid profile encryption key",
                ))
            }
        };
        let profile_signature_sk_type = match string_to_signature_secret_key(&profile_signature_sk) {
            Ok(key) => key,
            Err(_) => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "Invalid profile signature key",
                ))
            }
        };

        let my_subidentity_signature_pk = ed25519_dalek::PublicKey::from(&my_subidentity_signature_sk_type);
        let my_subidentity_encryption_pk = x25519_dalek::PublicKey::from(&my_subidentity_encryption_sk_type);
        let profile_signature_pk = ed25519_dalek::PublicKey::from(&profile_signature_sk_type);
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

        let body = match serde_json::to_string(&registration_code) {
            Ok(body) => body,
            Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
        };
        let other = encryption_public_key_to_string(my_subidentity_encryption_pk);

        Python::with_gil(|py| {
            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::TextContent,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create schema",
                    ))
                }
            };
            Self::create_custom_shinkai_message_to_node(
                my_device_encryption_sk,
                my_device_signature_sk,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                recipient.clone(),
                recipient,
                other,
                schema,
            )
        })
    }

    #[staticmethod]
    fn get_last_messages_from_inbox(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        inbox: String,
        count: usize,
        sender: String,
        sender_subidentity: String,
        recipient: String,
        recipient_subidentity: String,
        offset: Option<String>,
    ) -> PyResult<String> {
        let inbox_name = match InboxName::new(inbox.clone()) {
            Ok(name) => name,
            Err(_) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid inbox name")),
        };
        let get_last_messages_from_inbox = APIGetMessagesFromInboxRequest {
            inbox: inbox_name.to_string(),
            count,
            offset,
        };

        let body = match serde_json::to_string(&get_last_messages_from_inbox) {
            Ok(body) => body,
            Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
        };

        Python::with_gil(|py| {
            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::APIGetMessagesFromInboxRequest,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create schema",
                    ))
                }
            };
            Self::create_custom_shinkai_message_to_node(
                my_subidentity_encryption_sk,
                my_subidentity_signature_sk,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                recipient.clone(),
                recipient,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    fn get_last_unread_messages_from_inbox(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        inbox: String,
        count: usize,
        sender: String,
        sender_subidentity: String,
        recipient: String,
        recipient_subidentity: String,
        offset: Option<String>,
    ) -> PyResult<String> {
        let inbox_name = match InboxName::new(inbox.clone()) {
            Ok(name) => name,
            Err(_) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid inbox name")),
        };
        let get_last_unread_messages_from_inbox = APIGetMessagesFromInboxRequest {
            inbox: inbox_name.to_string(),
            count,
            offset,
        };

        let body = match serde_json::to_string(&get_last_unread_messages_from_inbox) {
            Ok(body) => body,
            Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
        };

        Python::with_gil(|py| {
            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::APIGetMessagesFromInboxRequest,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create schema",
                    ))
                }
            };
            Self::create_custom_shinkai_message_to_node(
                my_subidentity_encryption_sk,
                my_subidentity_signature_sk,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                recipient.clone(),
                recipient,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    fn request_add_agent(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        agent_json: &str,
        sender: String,
        sender_subidentity: String,
        recipient: String,
        recipient_subidentity: String,
    ) -> PyResult<String> {
        let agent: SerializedAgent = match serde_json::from_str(agent_json) {
            Ok(agent) => agent,
            Err(_) => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "Failed to deserialize agent from JSON",
                ))
            }
        };

        let add_agent_request = APIAddAgentRequest { agent };
        let body = match serde_json::to_string(&add_agent_request) {
            Ok(body) => body,
            Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
        };

        Python::with_gil(|py| {
            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::APIAddAgentRequest,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create schema",
                    ))
                }
            };
            Self::create_custom_shinkai_message_to_node(
                my_subidentity_encryption_sk,
                my_subidentity_signature_sk,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                recipient.clone(),
                recipient,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    fn read_up_to_time(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        inbox: String,
        up_to_time: String,
        sender: String,
        sender_subidentity: String,
        recipient: String,
        recipient_subidentity: String,
    ) -> PyResult<String> {
        let inbox_name = match InboxName::new(inbox.clone()) {
            Ok(name) => name,
            Err(_) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid inbox name")),
        };
        let read_up_to_time = APIReadUpToTimeRequest { inbox_name, up_to_time };

        let body = match serde_json::to_string(&read_up_to_time) {
            Ok(body) => body,
            Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
        };

        Python::with_gil(|py| {
            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::APIReadUpToTimeRequest,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create schema",
                    ))
                }
            };
            Self::create_custom_shinkai_message_to_node(
                my_subidentity_encryption_sk,
                my_subidentity_signature_sk,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                recipient.clone(),
                recipient,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    fn ping_pong_message(
        message: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: String,
        receiver: String,
    ) -> PyResult<String> {
        if message != "Ping" && message != "Pong" {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "Invalid message: must be 'Ping' or 'Pong'",
            ));
        }

        let mut builder = match PyShinkaiMessageBuilder::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
        ) {
            Ok(builder) => builder,
            Err(e) => return Err(e),
        };

        let _ = builder.message_raw_content(message);
        let _ = builder.empty_non_encrypted_internal_metadata();
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata(receiver, sender);

        builder.build_to_string()
    }

    #[staticmethod]
    fn job_creation(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        scope: Py<PyAny>,
        sender: String,
        receiver: String,
        receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let scope: PyJobScope = match scope.extract(py) {
                Ok(scope) => scope,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to deserialize scope from JSON",
                    ))
                }
            };

            let job_creation = JobCreationInfo { scope: scope.inner };
            let body = match serde_json::to_string(&job_creation) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let mut builder = match PyShinkaiMessageBuilder::new(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
            ) {
                Ok(builder) => builder,
                Err(e) => return Err(e),
            };

            let message_schema = match Py::new(py, PyMessageSchemaType::new("JobCreationSchema".to_string())?) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            let encryption = match Py::new(py, PyEncryptionMethod::new(Some("None"))) {
                Ok(encryption) => encryption,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create encryption method",
                    ))
                }
            };

            let _ = builder.message_raw_content(body);
            let _ = builder.internal_metadata_with_schema(
                "".to_string(),
                receiver_subidentity.clone(),
                "".to_string(),
                message_schema,
                encryption,
            );
            let _ = builder.no_body_encryption();
            let _ = builder.external_metadata(receiver, sender);

            builder.build_to_string()
        })
    }

    #[staticmethod]
    pub fn job_message(
        job_id: String,
        content: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: String,
        receiver: String,
        receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let job_id_clone = job_id.clone();
            let job_message = JobMessage { job_id, content, files_inbox: "".to_string() };

            let body = match serde_json::to_string(&job_message) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let mut builder = match PyShinkaiMessageBuilder::new(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
            ) {
                Ok(builder) => builder,
                Err(e) => return Err(e),
            };

            let inbox = match InboxName::get_job_inbox_name_from_params(job_id_clone) {
                Ok(inbox) => inbox.to_string(),
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let message_schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::JobMessageSchema,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            let encryption = match Py::new(py, PyEncryptionMethod::new(Some("None"))) {
                Ok(encryption) => encryption,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create encryption method",
                    ))
                }
            };

            let _ = builder.message_raw_content(body);
            let _ = builder.internal_metadata_with_schema(
                "".to_string(),
                receiver_subidentity.clone(),
                inbox,
                message_schema,
                encryption,
            );
            let _ = builder.no_body_encryption();
            let _ = builder.external_metadata(receiver, sender);

            builder.build_to_string()
        })
    }

    #[staticmethod]
pub fn terminate_message(
    my_encryption_secret_key: String,
    my_signature_secret_key: String,
    receiver_public_key: String,
    sender: String,
    receiver: String,
) -> PyResult<String> {
    Python::with_gil(|py| {
        let mut builder = match PyShinkaiMessageBuilder::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
        ) {
            Ok(builder) => builder,
            Err(e) => return Err(e),
        };

        let _ = builder.message_raw_content("terminate".to_string());
        let _ = builder.empty_non_encrypted_internal_metadata();
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata(receiver, sender);

        builder.build_to_string()
    })
}

#[staticmethod]
pub fn error_message(
    my_encryption_secret_key: String,
    my_signature_secret_key: String,
    receiver_public_key: String,
    sender: String,
    receiver: String,
    error_msg: String,
) -> PyResult<String> {
    Python::with_gil(|py| {
        let mut builder = match PyShinkaiMessageBuilder::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
        ) {
            Ok(builder) => builder,
            Err(e) => return Err(e),
        };

        let _ = builder.message_raw_content(format!("{{error: \"{}\"}}", error_msg));
        let _ = builder.empty_encrypted_internal_metadata();
        let _ = builder.no_body_encryption();
        let _ = builder.external_metadata(receiver, sender);

        builder.build_to_string()
    })
}
}
