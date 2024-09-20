use crate::shinkai_pyo3_utils::pyo3_job_scope::PyJobScope;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

use super::{
    encryption_method_pyo3::PyEncryptionMethod, message_schema_type_pyo3::PyMessageSchemaType,
    shinkai_message_pyo3::PyShinkaiMessage,
};
use pyo3::{prelude::*, pyclass, PyResult};
use shinkai_message_primitives::schemas::shinkai_subscription_req::{
    FolderSubscription, PaymentOption, SubscriptionPayment,
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIAvailableSharedItems, APISubscribeToSharedFolder, APIUnsubscribeToSharedFolder,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiNameString;
use shinkai_message_primitives::{
    schemas::{llm_providers::serialized_llm_provider::SerializedLLMProvider, inbox_name::InboxName, registration_code::RegistrationCodeSimple},
    shinkai_message::shinkai_message_schemas::{
        APIAddAgentRequest, APIConvertFilesAndSaveToFolder, APICreateShareableFolder, APIGetMessagesFromInboxRequest,
        APIReadUpToTimeRequest, APIVecFSRetrieveVectorResource, APIVecFsCopyFolder, APIVecFsCopyItem,
        APIVecFsCreateFolder, APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson,
        APIVecFsRetrieveVectorSearchSimplifiedJson, IdentityPermissions, JobCreationInfo, JobMessage,
        MessageSchemaType, RegistrationCodeRequest, RegistrationCodeType,
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

#[pyclass]
pub struct PyShinkaiMessageBuilder {
    pub inner: Option<ShinkaiMessageBuilder>,
}

#[pymethods]
impl PyShinkaiMessageBuilder {
    #[new]
    #[pyo3(text_signature = "(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)")]
    pub fn new(
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

    pub fn body_encryption(&mut self, encryption: Py<PyEncryptionMethod>) -> PyResult<()> {
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

    pub fn no_body_encryption(&mut self) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.no_body_encryption();
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    pub fn message_raw_content(&mut self, message_raw_content: String) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.message_raw_content(message_raw_content);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    pub fn message_schema_type(&mut self, content: Py<PyMessageSchemaType>) -> PyResult<()> {
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

    pub fn internal_metadata(
        &mut self,
        sender_subidentity: ShinkaiNameString,
        recipient_subidentity: String,
        encryption: Py<PyEncryptionMethod>,
    ) -> PyResult<()> {
        Python::with_gil(|py| {
            let encryption_ref = encryption.as_ref(py).borrow();
            if let Some(inner) = self.inner.take() {
                let new_inner = inner.internal_metadata(
                    sender_subidentity,
                    recipient_subidentity,
                    encryption_ref.inner.clone(),
                    None,
                );
                self.inner = Some(new_inner);
                Ok(())
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
            }
        })
    }

    pub fn internal_metadata_with_inbox(
        &mut self,
        sender_subidentity: ShinkaiNameString,
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
                    None,
                );
                self.inner = Some(new_inner);
                Ok(())
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
            }
        })
    }

    pub fn internal_metadata_with_schema(
        &mut self,
        sender_subidentity: ShinkaiNameString,
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
                    None,
                );
                self.inner = Some(new_inner);
                Ok(())
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
            }
        })
    }

    pub fn empty_encrypted_internal_metadata(&mut self) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.empty_encrypted_internal_metadata();
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    pub fn empty_non_encrypted_internal_metadata(&mut self) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.empty_non_encrypted_internal_metadata();
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    pub fn external_metadata(&mut self, recipient: String, sender: String) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.external_metadata(recipient, sender);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    pub fn external_metadata_with_intra_sender(
        &mut self,
        recipient: String,
        sender: String,
        intra_sender: String,
    ) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.external_metadata_with_intra_sender(recipient, sender, intra_sender);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    pub fn external_metadata_with_other(&mut self, recipient: String, sender: String, other: String) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.external_metadata_with_other(recipient, sender, other);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    pub fn external_metadata_with_other_and_intra_sender(
        &mut self,
        recipient: String,
        sender: String,
        other: String,
        intra_sender: String,
    ) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.external_metadata_with_other_and_intra_sender(recipient, sender, other, intra_sender);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    pub fn external_metadata_with_schedule(
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

    pub fn build(&mut self) -> PyResult<PyShinkaiMessage> {
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

    pub fn build_to_string(&mut self) -> PyResult<String> {
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
        sender_subidentity: ShinkaiNameString,
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

                    match builder.external_metadata_with_other_and_intra_sender(
                        recipient,
                        sender,
                        other,
                        sender_subidentity.clone(),
                    ) {
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
        sender_subidentity: ShinkaiNameString,
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
        sender_subidentity: ShinkaiNameString,
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

        let profile_signature_pk = profile_signature_sk_type.verifying_key();
        let profile_encryption_pk = x25519_dalek::PublicKey::from(&profile_encryption_sk_type);

        let registration_code = RegistrationCodeSimple {
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
        sender_subidentity: ShinkaiNameString,
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

        let my_subidentity_signature_pk = my_subidentity_signature_sk_type.verifying_key();
        let my_subidentity_encryption_pk = x25519_dalek::PublicKey::from(&my_subidentity_encryption_sk_type);
        let profile_signature_pk = profile_signature_sk_type.verifying_key();
        let profile_encryption_pk = x25519_dalek::PublicKey::from(&profile_encryption_sk_type);

        let other = encryption_public_key_to_string(my_subidentity_encryption_pk);
        let registration_code = RegistrationCodeSimple {
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
    #[allow(clippy::too_many_arguments)]
    fn initial_registration_with_no_code_for_device(
        my_device_encryption_sk: String,
        my_device_signature_sk: String,
        profile_encryption_sk: String,
        profile_signature_sk: String,
        registration_name: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        recipient: String,
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

        let my_subidentity_signature_pk = my_subidentity_signature_sk_type.verifying_key();
        let my_subidentity_encryption_pk = x25519_dalek::PublicKey::from(&my_subidentity_encryption_sk_type);
        let profile_signature_pk = profile_signature_sk_type.verifying_key();
        let profile_encryption_pk = x25519_dalek::PublicKey::from(&profile_encryption_sk_type);

        let identity_type = "device".to_string();
        let permission_type = "admin".to_string();

        let other = encryption_public_key_to_string(my_subidentity_encryption_pk);
        let registration_code = RegistrationCodeSimple {
            code: "".to_string(),
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
                    inner: MessageSchemaType::UseRegistrationCode,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create schema",
                    ))
                }
            };

            let builder_result =
                PyShinkaiMessageBuilder::new(my_device_encryption_sk, my_device_signature_sk, other.clone());

            match builder_result {
                Ok(mut builder) => {
                    let body_encryption = Py::new(
                        py,
                        PyEncryptionMethod {
                            inner: EncryptionMethod::None,
                        },
                    )?;
                    let internal_encryption = Py::new(
                        py,
                        PyEncryptionMethod {
                            inner: EncryptionMethod::None,
                        },
                    )?;

                    match builder.message_raw_content(body) {
                        Ok(_) => (),
                        Err(e) => return Err(e),
                    }

                    match builder.body_encryption(body_encryption) {
                        Ok(_) => (),
                        Err(e) => return Err(e),
                    }

                    match builder.external_metadata_with_other_and_intra_sender(
                        recipient,
                        sender,
                        other,
                        sender_subidentity.clone(),
                    ) {
                        Ok(_) => (),
                        Err(e) => return Err(e),
                    }

                    match builder.internal_metadata_with_schema(
                        sender_subidentity,
                        "".to_string(),
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
    #[allow(clippy::too_many_arguments)]
    fn get_last_messages_from_inbox(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        inbox: String,
        count: usize,
        sender: String,
        sender_subidentity: ShinkaiNameString,
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
                recipient,
                recipient_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    fn get_last_unread_messages_from_inbox(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        inbox: String,
        count: usize,
        sender: String,
        sender_subidentity: ShinkaiNameString,
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
                recipient,
                recipient_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    fn request_add_agent(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        agent_json: &str,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        recipient: String,
        recipient_subidentity: String,
    ) -> PyResult<String> {
        let agent: SerializedLLMProvider = match serde_json::from_str(agent_json) {
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
                recipient,
                recipient_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    fn read_up_to_time(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        inbox: String,
        up_to_time: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
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
    #[allow(clippy::too_many_arguments)]
    pub fn create_files_inbox_with_sym_key(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        inbox: String,
        symmetric_key_sk: String,
        sender_subidentity: ShinkaiNameString,
        sender: String,
        receiver: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let mut builder = match PyShinkaiMessageBuilder::new(
                my_subidentity_encryption_sk,
                my_subidentity_signature_sk,
                receiver_public_key,
            ) {
                Ok(builder) => builder,
                Err(e) => return Err(e),
            };

            let outer_encryption = match Py::new(
                py,
                PyEncryptionMethod {
                    inner: EncryptionMethod::DiffieHellmanChaChaPoly1305,
                },
            ) {
                Ok(encryption) => encryption,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create encryption method",
                    ))
                }
            };

            let inner_encryption = match Py::new(
                py,
                PyEncryptionMethod {
                    inner: EncryptionMethod::None,
                },
            ) {
                Ok(encryption) => encryption,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create encryption method",
                    ))
                }
            };

            let schema = MessageSchemaType::SymmetricKeyExchange.to_str();
            let message_schema = match Py::new(py, PyMessageSchemaType::new(schema.to_string())?) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            let _ = builder.message_raw_content(symmetric_key_sk);
            let _ = builder.body_encryption(outer_encryption);
            let _ = builder.internal_metadata_with_schema(
                sender_subidentity.clone(),
                "".to_string(),
                inbox.to_string(),
                message_schema,
                inner_encryption,
            );
            let _ = builder.external_metadata_with_intra_sender(receiver.clone(), sender, sender_subidentity);

            builder.build_to_string()
        })
    }

    #[staticmethod]
    pub fn get_all_inboxes_for_profile(
        my_subidentity_encryption_sk: String,
        my_subidentity_signature_sk: String,
        receiver_public_key: String,
        full_profile: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let mut builder = match PyShinkaiMessageBuilder::new(
                my_subidentity_encryption_sk,
                my_subidentity_signature_sk,
                receiver_public_key,
            ) {
                Ok(builder) => builder,
                Err(e) => return Err(e),
            };

            let _ = builder.message_raw_content(full_profile);

            let inner_encryption = match Py::new(
                py,
                PyEncryptionMethod {
                    inner: EncryptionMethod::None,
                },
            ) {
                Ok(encryption) => encryption,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create encryption method",
                    ))
                }
            };

            let schema = MessageSchemaType::TextContent.to_str();
            let message_schema = match Py::new(py, PyMessageSchemaType::new(schema.to_string())?) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            let _ = builder.internal_metadata_with_schema(
                sender_subidentity.clone(),
                "".to_string(),
                "".to_string(),
                message_schema,
                inner_encryption,
            );

            let outer_encryption = match Py::new(
                py,
                PyEncryptionMethod {
                    inner: EncryptionMethod::DiffieHellmanChaChaPoly1305,
                },
            ) {
                Ok(encryption) => encryption,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create encryption method",
                    ))
                }
            };

            let _ = builder.body_encryption(outer_encryption);
            let _ = builder.external_metadata_with_intra_sender(receiver.clone(), sender, sender_subidentity);
            builder.build_to_string()
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
    #[allow(clippy::too_many_arguments)]
    fn job_creation(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        scope: Py<PyJobScope>,
        is_hidden: bool,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        node_receiver: String,
        node_receiver_subidentity: String,
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
            let job_creation = JobCreationInfo {
                scope: scope.inner.clone(),
                is_hidden: Some(is_hidden),
                associated_ui: None,
            };

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
                sender_subidentity.clone(),
                node_receiver_subidentity.clone(),
                "".to_string(),
                message_schema,
                encryption,
            );
            let _ = builder.no_body_encryption();
            let _ = builder.external_metadata_with_intra_sender(node_receiver, sender, sender_subidentity);

            builder.build_to_string()
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    pub fn job_message(
        job_id: String,
        content: String,
        files_inbox: String,
        parent: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
        receiver_subidentity: String,
        workflow_code: Option<String>,
        workflow_name: Option<String>
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let job_id_clone = job_id.clone();
            let job_message = JobMessage {
                job_id,
                content,
                files_inbox,
                parent: Some(parent),
                workflow_code,
                workflow_name,
                sheet_job_data: None,
                callback: None,
            };

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
                sender_subidentity.to_string(),
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

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_create_folder(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        folder_name: String,
        path: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
        receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let payload = APIVecFsCreateFolder {
                path: path.clone(),
                folder_name: folder_name.clone(),
            };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::VecFsCreateFolder,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                receiver,
                receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_move_folder(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        origin_path: String,
        destination_path: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
        receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let payload = APIVecFsMoveFolder {
                origin_path: origin_path.clone(),
                destination_path: destination_path.clone(),
            };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::VecFsMoveFolder,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                receiver,
                receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_copy_folder(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        origin_path: String,
        destination_path: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
        receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let payload = APIVecFsCopyFolder {
                origin_path: origin_path.clone(),
                destination_path: destination_path.clone(),
            };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::VecFsCopyFolder,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                receiver,
                receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_move_item(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        origin_path: String,
        destination_path: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
        receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let payload = APIVecFsMoveItem {
                origin_path: origin_path.clone(),
                destination_path: destination_path.clone(),
            };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::VecFsMoveItem,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                receiver,
                receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_copy_item(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        origin_path: String,
        destination_path: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
        receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let payload = APIVecFsCopyItem {
                origin_path: origin_path,
                destination_path: destination_path,
            };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::VecFsCopyItem,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                receiver,
                receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_create_items(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        destination_path: String,
        file_inbox: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
        receiver_subidentity: String,
        file_datetime_iso8601: Option<String>,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let file_datetime_option = file_datetime_iso8601.and_then(|dt| {
                chrono::DateTime::parse_from_rfc3339(&dt)
                    .map(|parsed_dt| parsed_dt.with_timezone(&chrono::Utc))
                    .ok()
            });

            let payload = APIConvertFilesAndSaveToFolder {
                path: destination_path,
                file_inbox,
                file_datetime: file_datetime_option,
            };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::ConvertFilesAndSaveToFolder,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                receiver,
                receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_retrieve_resource(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        path: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
        receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let payload = APIVecFSRetrieveVectorResource { path };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::VecFsRetrieveVectorResource,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                receiver,
                receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_retrieve_path_simplified(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        path: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
        receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let payload = APIVecFsRetrievePathSimplifiedJson { path };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                receiver,
                receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_retrieve_vector_search_simplified(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        search: String,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
        receiver_subidentity: String,
        path: Option<String>,
        max_results: Option<usize>,
        max_files_to_scan: Option<usize>,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let payload = APIVecFsRetrieveVectorSearchSimplifiedJson {
                search,
                path,
                max_results,
                max_files_to_scan,
            };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string().clone())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::VecFsRetrieveVectorSearchSimplifiedJson,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                receiver,
                receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
    my_encryption_secret_key,
    my_signature_secret_key,
    receiver_public_key,
    path,
    folder_description,
    is_free,
    has_web_alternative,
    sender,
    sender_subidentity,
    receiver,
    receiver_subidentity,
    minimum_token_delegation = None,
    minimum_time_delegated_hours = None,
    monthly_payment_usd = None,
    monthly_payment_kai_tokens = None,
))]
    pub fn subscriptions_create_share_folder(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        path: String,
        folder_description: String,
        is_free: bool,
        has_web_alternative: bool,
        sender: String,
        sender_subidentity: ShinkaiNameString,
        receiver: String,
        receiver_subidentity: String,
        minimum_token_delegation: Option<u64>,
        minimum_time_delegated_hours: Option<u64>,
        monthly_payment_usd: Option<f64>,
        monthly_payment_kai_tokens: Option<u64>,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let payload = APICreateShareableFolder {
                path,
                subscription_req: FolderSubscription {
                    minimum_token_delegation,
                    minimum_time_delegated_hours,
                    monthly_payment: match (monthly_payment_usd, monthly_payment_kai_tokens) {
                        (Some(usd), None) => {
                            Some(PaymentOption::USD(Decimal::from_f64(usd).ok_or_else(|| {
                                PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid USD value")
                            })?))
                        }
                        (None, Some(tokens)) => Some(PaymentOption::KAITokens(Decimal::from_u64(tokens).ok_or_else(
                            || PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid KAI Tokens value"),
                        )?)),
                        (None, None) => None,
                        _ => {
                            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                                "Invalid monthly payment options",
                            ))
                        }
                    },
                    has_web_alternative: Some(has_web_alternative),
                    is_free,
                    folder_description,
                },
                credentials: None,
            };

            let body = serde_json::to_string(&payload)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

            let schema = Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::CreateShareableFolder,
                },
            )
            .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("Failed to create message schema"))?;

            PyShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                receiver,
                receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        my_encryption_secret_key,
        my_signature_secret_key,
        receiver_public_key,
        results,
        sender,
        sender_subidentity,
        node_receiver,
        node_receiver_subidentity
    ))]
    pub fn vecfs_available_shared_items_response(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        results: String,
        sender: String,
        sender_subidentity: String,
        node_receiver: String,
        node_receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::AvailableSharedItemsResponse,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                results,
                sender,
                sender_subidentity,
                node_receiver,
                node_receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[staticmethod]
    pub fn vecfs_available_shared_items(
        streamer_node_name: String,
        streamer_profile_name: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: String,
        sender_subidentity: String,
        node_receiver: String,
        node_receiver_subidentity: String,
        path: Option<String>,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let payload = APIAvailableSharedItems {
                path: path.unwrap_or_else(|| "/".to_string()),
                streamer_node_name,
                streamer_profile_name,
            };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::AvailableSharedItems,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                node_receiver,
                node_receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[staticmethod]
    pub fn vecfs_subscribe_to_shared_folder(
        shared_folder: String,
        requirements_free: bool,
        streamer_node: String,
        streamer_profile: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: String,
        sender_subidentity: String,
        node_receiver: String,
        node_receiver_subidentity: String,
        requirements_direct_delegation: bool,
        http_preferred: Option<bool>,
        base_folder: Option<String>,
        requirements_payment: Option<String>,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let requirements = if requirements_free {
                SubscriptionPayment::Free
            } else if requirements_direct_delegation {
                SubscriptionPayment::DirectDelegation
            } else if let Some(payment) = requirements_payment {
                SubscriptionPayment::Payment(payment)
            } else {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "Invalid subscription payment requirements",
                ));
            };

            let payload = APISubscribeToSharedFolder {
                path: shared_folder,
                streamer_node_name: streamer_node,
                streamer_profile_name: streamer_profile,
                payment: requirements,
                http_preferred,
                base_folder,
            };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::SubscribeToSharedFolder,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                node_receiver,
                node_receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[staticmethod]
    pub fn vecfs_unsubscribe_to_shared_folder(
        shared_folder: String,
        streamer_node: String,
        streamer_profile: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: String,
        sender_subidentity: String,
        node_receiver: String,
        node_receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let payload = APIUnsubscribeToSharedFolder {
                path: shared_folder,
                streamer_node_name: streamer_node,
                streamer_profile_name: streamer_profile,
            };

            let body = match serde_json::to_string(&payload) {
                Ok(body) => body,
                Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())),
            };

            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::UnsubscribeToSharedFolder,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                body,
                sender,
                sender_subidentity,
                node_receiver,
                node_receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[staticmethod]
    pub fn my_subscriptions(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: String,
        sender_subidentity: String,
        node_receiver: String,
        node_receiver_subidentity: String,
    ) -> PyResult<String> {
        Python::with_gil(|py| {
            let schema = match Py::new(
                py,
                PyMessageSchemaType {
                    inner: MessageSchemaType::MySubscriptions,
                },
            ) {
                Ok(schema) => schema,
                Err(_) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "Failed to create message schema",
                    ))
                }
            };

            Self::create_custom_shinkai_message_to_node(
                my_encryption_secret_key,
                my_signature_secret_key,
                receiver_public_key,
                "".to_string(),
                sender,
                sender_subidentity,
                node_receiver,
                node_receiver_subidentity,
                "".to_string(),
                schema,
            )
        })
    }
}
