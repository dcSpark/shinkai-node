use std::str::FromStr;
use pyo3::exceptions::PyValueError;
use pyo3::{ToPyObject, PyObject, Python, PyErr, pyclass};
use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
use pyo3::prelude::*;

#[pyclass]
#[derive(Clone)]
pub struct PyEncryptionMethod {
    pub inner: EncryptionMethod,
}

#[pymethods]
impl PyEncryptionMethod {
    #[new]
    pub fn new(value: Option<&str>) -> Self {
        let value = value.unwrap_or("None");
        let inner = match value {
            "None" => EncryptionMethod::None,
            // Add other cases here for other possible values of EncryptionMethod
            _ => panic!("Invalid value for EncryptionMethod"),
        };
        PyEncryptionMethod { inner }
    }
}

impl FromStr for PyEncryptionMethod {
    type Err = pyo3::PyErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let encryption = match s {
            "DiffieHellmanChaChaPoly1305" | "default" => EncryptionMethod::DiffieHellmanChaChaPoly1305,
            "None" => EncryptionMethod::None,
            _ => return Err(PyValueError::new_err("Invalid EncryptionMethod")),
        };
        Ok(PyEncryptionMethod { inner: encryption })
    }
}

impl ToPyObject for PyEncryptionMethod {
    fn to_object(&self, py: Python) -> PyObject {
        match self.inner {
            EncryptionMethod::DiffieHellmanChaChaPoly1305 => "default".to_object(py),
            EncryptionMethod::None => "None".to_object(py),
        }
    }
}