use pyo3::{prelude::*, pyclass, types::PyDict, PyResult};
use shinkai_message_primitives::shinkai_utils::{
    encryption::{string_to_encryption_public_key, string_to_encryption_static_key},
    shinkai_message_builder::ShinkaiMessageBuilder,
    signatures::string_to_signature_secret_key,
};

use super::{encryption_method_pyo3::PyEncryptionMethod, shinkai_message_pyo3::PyShinkaiMessage, message_schema_type_pyo3::PyMessageSchemaType};

#[pyclass]
pub struct PyShinkaiMessageBuilder {
    // pub inner: ShinkaiMessageBuilder,
    pub inner: Option<ShinkaiMessageBuilder>,
}

#[pymethods]
impl PyShinkaiMessageBuilder {
    #[new]
    #[args(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)]
    fn new(my_encryption_secret_key: String, my_signature_secret_key: String, receiver_public_key: String) -> PyResult<Self> {
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

    fn internal_metadata(&mut self, sender_subidentity: String, recipient_subidentity: String, encryption: Py<PyEncryptionMethod>) -> PyResult<()> {
        Python::with_gil(|py| {
            let encryption_ref = encryption.as_ref(py).borrow();
            if let Some(inner) = self.inner.take() {
                let new_inner = inner.internal_metadata(sender_subidentity, recipient_subidentity, encryption_ref.inner.clone());
                self.inner = Some(new_inner);
                Ok(())
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
            }
        })
    }

    fn internal_metadata_with_inbox(&mut self, sender_subidentity: String, recipient_subidentity: String, inbox: String, encryption: Py<PyEncryptionMethod>) -> PyResult<()> {
        Python::with_gil(|py| {
            let encryption_ref = encryption.as_ref(py).borrow();
            if let Some(inner) = self.inner.take() {
                let new_inner = inner.internal_metadata_with_inbox(sender_subidentity, recipient_subidentity, inbox, encryption_ref.inner.clone());
                self.inner = Some(new_inner);
                Ok(())
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
            }
        })
    }

    fn internal_metadata_with_schema(&mut self, sender_subidentity: String, recipient_subidentity: String, inbox: String, message_schema: Py<PyMessageSchemaType>, encryption: Py<PyEncryptionMethod>) -> PyResult<()> {
        Python::with_gil(|py| {
            let encryption_ref = encryption.as_ref(py).borrow();
            let message_schema_ref = message_schema.as_ref(py).borrow();
            if let Some(inner) = self.inner.take() {
                let new_inner = inner.internal_metadata_with_schema(sender_subidentity, recipient_subidentity, inbox, message_schema_ref.inner.clone(), encryption_ref.inner.clone());
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

    fn external_metadata_with_other(&mut self, recipient: String, sender: String, other: String) -> PyResult<()> {
        if let Some(inner) = self.inner.take() {
            let new_inner = inner.external_metadata_with_other(recipient, sender, other);
            self.inner = Some(new_inner);
            Ok(())
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    fn external_metadata_with_schedule(&mut self, recipient: String, sender: String, scheduled_time: String) -> PyResult<()> {
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
                },
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
                    serde_json::to_string(&shinkai_message).map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
                },
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())),
            }
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("inner is None"))
        }
    }

    #[staticmethod]
    fn ack_message(my_encryption_secret_key: String, my_signature_secret_key: String, receiver_public_key: String, sender: String, receiver: String) -> PyResult<String> {
        let mut builder = PyShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)?;

        builder.message_raw_content("ACK".to_string())?;
        builder.empty_non_encrypted_internal_metadata()?;
        builder.no_body_encryption()?;
        builder.external_metadata(receiver, sender)?;
        builder.build_to_string()
    }

    
}
