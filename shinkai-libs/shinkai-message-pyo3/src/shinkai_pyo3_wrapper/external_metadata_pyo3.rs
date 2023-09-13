use pyo3::{exceptions::PyValueError, prelude::*, types::PyDict};
use shinkai_message_primitives::{
    shinkai_message::shinkai_message::{
        EncryptedShinkaiBody, ExternalMetadata, MessageBody, ShinkaiBody, ShinkaiMessage, ShinkaiVersion,
    },
    shinkai_utils::encryption::EncryptionMethod,
};

use super::shinkai_message_pyo3::PyExternalMetadata;

#[pymethods]
impl PyExternalMetadata {
    #[new]
    fn new(sender: String, recipient: String, scheduled_time: String, signature: String, other: String) -> PyResult<Self> {
        let inner = ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other,
        };

        Ok(Self { inner })
    }

    #[getter]
    fn get_sender(&self) -> PyResult<String> {
        Ok(self.inner.sender.clone())
    }

    #[setter]
    fn set_sender(&mut self, sender: String) {
        self.inner.sender = sender;
    }

    #[getter]
    fn get_recipient(&self) -> PyResult<String> {
        Ok(self.inner.recipient.clone())
    }

    #[setter]
    fn set_recipient(&mut self, recipient: String) {
        self.inner.recipient = recipient;
    }

    #[getter]
    fn get_scheduled_time(&self) -> PyResult<String> {
        Ok(self.inner.scheduled_time.clone())
    }

    #[setter]
    fn set_scheduled_time(&mut self, scheduled_time: String) {
        self.inner.scheduled_time = scheduled_time;
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
    fn get_other(&self) -> PyResult<String> {
        Ok(self.inner.other.clone())
    }

    #[setter]
    fn set_other(&mut self, other: String) {
        self.inner.other = other;
    }
}

impl<'source> FromPyObject<'source> for PyExternalMetadata {
    fn extract(ob: &'source PyAny) -> PyResult<Self> {
        let s: &str = ob.extract()?;
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 5 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "Invalid ExternalMetadata format",
            ));
        }
        let sender = parts[0].to_string();
        let recipient = parts[1].to_string();
        let scheduled_time = parts[2].to_string();
        let signature = parts[3].to_string();
        let other = parts[4].to_string();
        Ok(PyExternalMetadata {
            inner: ExternalMetadata {
                sender,
                recipient,
                scheduled_time,
                signature,
                other,
            },
        })
    }
}

impl ToPyObject for PyExternalMetadata {
    fn to_object(&self, py: Python) -> PyObject {
        let dict = PyDict::new(py);
        dict.set_item("sender", self.inner.sender.clone()).unwrap();
        dict.set_item("recipient", self.inner.recipient.clone()).unwrap();
        dict.set_item("scheduled_time", self.inner.scheduled_time.clone())
            .unwrap();
        dict.set_item("signature", self.inner.signature.clone()).unwrap();
        dict.set_item("other", self.inner.other.clone()).unwrap();
        dict.into()
    }
}
