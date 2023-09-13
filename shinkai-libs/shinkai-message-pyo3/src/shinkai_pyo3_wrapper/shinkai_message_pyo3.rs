use pyo3::{exceptions::PyValueError, prelude::*, types::PyDict};
use shinkai_message_primitives::{
    shinkai_message::shinkai_message::{
        EncryptedShinkaiBody, ExternalMetadata, MessageBody, ShinkaiBody, ShinkaiMessage, ShinkaiVersion, InternalMetadata,
    },
    shinkai_utils::encryption::EncryptionMethod,
};

#[pymodule]
fn shinkai_message_pyo3(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyShinkaiMessage>()?;
    m.add_class::<PyExternalMetadata>()?;
    m.add_class::<PyInternalMetadata>()?;
    // Add any other classes or functions you want to expose to Python here
    Ok(())
}

#[pyclass]
pub struct PyShinkaiMessage {
    pub inner: ShinkaiMessage,
}

#[pyclass]
pub struct PyExternalMetadata {
    pub inner: ExternalMetadata,
}

#[pyclass]
pub struct PyInternalMetadata {
    pub inner: InternalMetadata,
}

// #[pymethods]
// impl PyShinkaiMessage {
//     #[new]
//     #[args(kwargs = "**")]
//     fn new(kwargs: Option<&PyDict>) -> PyResult<Self> {
//         // Initialize your PyShinkaiMessage here using kwargs if necessary
//         // For now, let's just use the default
//         Ok(Self {
//             inner: Default::default(),
//         })
//     }

//     #[getter]
//     fn get_external_metadata(&self) -> PyResult<PyExternalMetadata> {
//         Ok(PyExternalMetadata {
//             inner: self.inner.external_metadata.clone(),
//         })
//     }

//     #[setter]
//     fn set_external_metadata(&mut self, py_external_metadata: PyExternalMetadata) {
//         self.inner.external_metadata = py_external_metadata.inner;
//     }

//     // ... rest of your methods here, replacing [self](file:///Users/nijaar/shinkai/develop/shinkai-node/src/network/node_error.rs#11%2C13-11%2C13) with `self.inner`
// }

// // Previous Approach 2

// #[pymethods]
// impl ShinkaiMessage {
//     #[getter]
//     fn get_body(&self) -> PyResult<MessageBody> {
//         Ok(self.body.clone())
//     }

//     #[setter]
//     fn set_body(&mut self, body: MessageBody) {
//         self.body = body;
//     }

//     #[getter]
//     fn get_external_metadata(&self) -> PyResult<ExternalMetadata> {
//         Ok(self.external_metadata.clone())
//     }

//     #[setter]
//     fn set_external_metadata(&mut self, external_metadata: ExternalMetadata) {
//         self.external_metadata = external_metadata;
//     }

//     #[getter]
//     fn get_encryption(&self) -> PyResult<EncryptionMethod> {
//         Ok(self.encryption.clone())
//     }

//     #[setter]
//     fn set_encryption(&mut self, encryption: EncryptionMethod) {
//         self.encryption = encryption;
//     }

//     #[getter]
//     fn get_version(&self) -> PyResult<ShinkaiVersion> {
//         Ok(self.version.clone())
//     }

//     #[setter]
//     fn set_version(&mut self, version: ShinkaiVersion) {
//         self.version = version;
//     }
// }

// Previous Approach
//
// #[pyclass]
// #[derive(Clone)]
// struct PyEncryptionMethod {
//     value: EncryptionMethod,
// }

// #[pymethods]
// impl PyEncryptionMethod {
//     #[new]
//     fn new(value: String) -> Self {
//         let method = match value.as_str() {
//             "DiffieHellmanChaChaPoly1305" | "default" => EncryptionMethod::DiffieHellmanChaChaPoly1305,
//             _ => EncryptionMethod::None,
//         };

//         PyEncryptionMethod { value: method }
//     }

//     #[getter]
//     fn value(&self) -> String {
//         match self.value {
//             EncryptionMethod::DiffieHellmanChaChaPoly1305 => String::from("DiffieHellmanChaChaPoly1305"),
//             EncryptionMethod::None => String::from("None"),
//         }
//     }
// }

// #[pyclass]
// #[derive(Clone)]
// struct PyShinkaiVersion {
//     value: ShinkaiVersion,
// }

// #[pyclass]
// #[derive(Clone)]
// pub struct PyMessageBody {
//     value: MessageBody,
// }

// #[pyclass]
// #[derive(Clone)]
// struct PyInternalMetadata {
//     #[pyo3(get, set)]
//     sender_subidentity: String,
//     #[pyo3(get, set)]
//     recipient_subidentity: String,
//     #[pyo3(get, set)]
//     inbox: String,
//     #[pyo3(get, set)]
//     signature: String,
//     #[pyo3(get, set)]
//     encryption: PyEncryptionMethod, // Assuming you have this class defined
// }

// #[pyclass]
// #[derive(Clone)]
// struct PyExternalMetadata {
//     #[pyo3(get, set)]
//     sender: String,
//     #[pyo3(get, set)]
//     recipient: String,
//     #[pyo3(get, set)]
//     scheduled_time: String,
//     #[pyo3(get, set)]
//     signature: String,
//     #[pyo3(get, set)]
//     other: String,
// }

// #[pyclass]
// #[derive(Clone)]
// struct PyShinkaiMessage {
//     #[pyo3(get, set)]
//     body: PyMessageBody,
//     #[pyo3(get, set)]
//     external_metadata: PyExternalMetadata,
//     #[pyo3(get, set)]
//     encryption: PyEncryptionMethod,
//     #[pyo3(get, set)]
//     version: PyShinkaiVersion,
// }

// #[pymodule]
// fn shinkai_message_pyo3(_py: Python, m: &PyModule) -> PyResult<()> {
//     #[pymethods]
//     impl PyInternalMetadata {
//         #[new]
//         fn new(
//             sender_subidentity: String,
//             recipient_subidentity: String,
//             inbox: String,
//             signature: String,
//             encryption: PyEncryptionMethod,
//         ) -> Self {
//             PyInternalMetadata {
//                 sender_subidentity,
//                 recipient_subidentity,
//                 inbox,
//                 signature,
//                 encryption,
//             }
//         }
//     }

//     #[pymethods]
//     impl PyExternalMetadata {
//         #[new]
//         fn new(sender: String, recipient: String, scheduled_time: String, signature: String, other: String) -> Self {
//             PyExternalMetadata {
//                 sender,
//                 recipient,
//                 scheduled_time,
//                 signature,
//                 other,
//             }
//         }
//     }

//     #[pymethods]
//     impl PyShinkaiVersion {
//         #[new]
//         #[args(value = "String::from(\"V1_0\")")]
//         fn new(value: String) -> Self {
//             let version = match value.as_str() {
//                 "V1_0" => ShinkaiVersion::V1_0,
//                 _ => ShinkaiVersion::Unsupported,
//             };

//             PyShinkaiVersion { value: version }
//         }

//         #[getter]
//         fn value(&self) -> String {
//             match self.value {
//                 ShinkaiVersion::V1_0 => String::from("V1_0"),
//                 ShinkaiVersion::Unsupported => String::from("Unsupported"),
//             }
//         }
//     }

//     Ok(())
// }

// #[pymethods]
// impl PyMessageBody {
//     #[new]
//     fn new(value: String, body: PyObject) -> PyResult<Self> {
//         let py = body.py();
//         let value = match value.as_str() {
//             "encrypted" => MessageBody::Encrypted(body.extract::<EncryptedShinkaiBody>(py)?),
//             "unencrypted" => MessageBody::Unencrypted(body.extract::<ShinkaiBody>(py)?),
//             _ => return Err(PyValueError::new_err("Invalid value")),
//         };

//         Ok(PyMessageBody { value })
//     }

//     #[getter]
//     fn value(&self) -> PyResult<String> {
//         match &self.value {
//             MessageBody::Encrypted(_) => Ok(String::from("encrypted")),
//             MessageBody::Unencrypted(_) => Ok(String::from("unencrypted")),
//         }
//     }

//     #[getter]
//     fn body(&self) -> PyResult<PyObject> {
//         let gil_guard = Python::acquire_gil();
//         let py = gil_guard.python();

//         match &self.value {
//             MessageBody::Encrypted(body) => Ok(body.into_py(py)),
//             MessageBody::Unencrypted(body) => Ok(body.into_py(py)),
//         }
//     }
// }
