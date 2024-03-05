use super::encryption_method_pyo3::PyEncryptionMethod;
use crate::shinkai_pyo3_wrapper::shinkai_message_pyo3::PyInternalMetadata;
use pyo3::ToPyObject;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyDict};
use shinkai_message_primitives::{
    shinkai_message::shinkai_message::InternalMetadata, shinkai_utils::encryption::EncryptionMethod,
};

#[pymethods]
impl PyInternalMetadata {
    #[new]
    fn new(
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        signature: String,
        encryption: Py<PyEncryptionMethod>,
    ) -> PyResult<Self> {
        Python::with_gil(|py| {
            let encryption_ref = encryption.as_ref(py).borrow();
            let inner = InternalMetadata {
                sender_subidentity,
                recipient_subidentity,
                inbox,
                signature,
                encryption: encryption_ref.inner.clone(),
                node_api_data: None,
            };
            Ok(Self { inner })
        })
    }

    #[getter]
    fn get_sender_subidentity(&self) -> PyResult<String> {
        Ok(self.inner.sender_subidentity.clone())
    }

    #[setter]
    fn set_sender_subidentity(&mut self, sender_subidentity: String) {
        self.inner.sender_subidentity = sender_subidentity;
    }

    #[getter]
    fn get_recipient_subidentity(&self) -> PyResult<String> {
        Ok(self.inner.recipient_subidentity.clone())
    }

    #[setter]
    fn set_recipient_subidentity(&mut self, recipient_subidentity: String) {
        self.inner.recipient_subidentity = recipient_subidentity;
    }

    #[getter]
    fn get_inbox(&self) -> PyResult<String> {
        Ok(self.inner.inbox.clone())
    }

    #[setter]
    fn set_inbox(&mut self, inbox: String) {
        self.inner.inbox = inbox;
    }

    #[getter]
    fn get_signature(&self) -> PyResult<String> {
        Ok(self.inner.signature.clone())
    }

    #[setter]
    fn set_signature(&mut self, signature: String) {
        self.inner.signature = signature;
    }

    #[getter]
    fn get_encryption(&self) -> PyResult<Py<PyEncryptionMethod>> {
        Python::with_gil(|py| {
            Ok(Py::new(
                py,
                PyEncryptionMethod {
                    inner: self.inner.encryption.clone(),
                },
            )?)
        })
    }

    #[setter]
    fn set_encryption(&mut self, encryption: Py<PyEncryptionMethod>) {
        Python::with_gil(|py| {
            let encryption_ref = encryption.as_ref(py).borrow();
            self.inner.encryption = encryption_ref.inner.clone();
        });
    }
}

impl<'source> FromPyObject<'source> for PyInternalMetadata {
    fn extract(ob: &'source PyAny) -> PyResult<Self> {
        let s: &str = ob.extract()?;
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 5 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "Invalid InternalMetadata format",
            ));
        }
        let sender_subidentity = parts[0].to_string();
        let recipient_subidentity = parts[1].to_string();
        let inbox = parts[2].to_string();
        let signature = parts[3].to_string();
        let encryption_str = parts[4].to_string();
        let encryption = EncryptionMethod::from_str(&encryption_str);

        Ok(PyInternalMetadata {
            inner: InternalMetadata {
                sender_subidentity,
                recipient_subidentity,
                inbox,
                signature,
                encryption,
                node_api_data: None,
            },
        })
    }
}

impl ToPyObject for PyInternalMetadata {
    fn to_object(&self, py: Python) -> PyObject {
        let dict = PyDict::new(py);
        dict.set_item("sender_subidentity", self.inner.sender_subidentity.clone())
            .unwrap();
        dict.set_item("recipient_subidentity", self.inner.recipient_subidentity.clone())
            .unwrap();
        dict.set_item("inbox", self.inner.inbox.clone()).unwrap();
        dict.set_item("signature", self.inner.signature.clone()).unwrap();
        dict.set_item("encryption", self.inner.encryption.as_str()).unwrap();
        dict.into()
    }
}
