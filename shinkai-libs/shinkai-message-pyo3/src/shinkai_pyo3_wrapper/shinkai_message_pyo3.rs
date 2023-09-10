use pyo3::prelude::*;
use shinkai_message_primitives::shinkai_message::shinkai_message::{ShinkaiMessage, MessageBody, ExternalMetadata, ShinkaiVersion};

#[pymodule]
fn shinkai_message_pyo3(_py: Python, m: &PyModule) -> PyResult<()> {
    #[pyclass]
    #[derive(Clone)]
    struct PyShinkaiMessage {
        #[pyo3(get, set)]
        body: PyMessageBody,
        #[pyo3(get, set)]
        external_metadata: PyExternalMetadata,
        #[pyo3(get, set)]
        encryption: PyEncryptionMethod,
        #[pyo3(get, set)]
        version: PyShinkaiVersion,
    }

    #[pyclass]
    #[derive(Clone)]
    struct PyMessageBody {
        // Define the fields of PyMessageBody here
    }

    #[pyclass]
    #[derive(Clone)]
    struct PyExternalMetadata {
        // Define the fields of PyExternalMetadata here
    }

    #[pyclass]
    #[derive(Clone)]
    struct PyEncryptionMethod {
        // Define the fields of PyEncryptionMethod here
    }

    #[pyclass]
    #[derive(Clone)]
    struct PyShinkaiVersion {
        value: ShinkaiVersion,
    }

    #[pymethods]
    impl PyShinkaiVersion {
        #[new]
        #[args(value = "String::from(\"V1_0\")")]
        #[text_signature = "(value)"]
        fn new(value: String) -> Self {
            let version = match value.as_str() {
                "V1_0" => ShinkaiVersion::V1_0,
                _ => ShinkaiVersion::Unsupported,
            };

            PyShinkaiVersion { value: version }
        }

        #[getter]
        fn value(&self) -> String {
            match self.value {
                ShinkaiVersion::V1_0 => String::from("V1_0"),
                ShinkaiVersion::Unsupported => String::from("Unsupported"),
            }
        }
    }
    
    Ok(())
}