use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::types::PyString;
use pyo3::wrap_pyfunction;
use serde_json::Error as SerdeError;
use shinkai_message_primitives::schemas::agents::serialized_llm_provider::AgentLLMInterface;
use shinkai_message_primitives::schemas::agents::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use std::str::FromStr;

use super::pyo3_llm_provider_interface::PyAgentLLMInterface;
use super::pyo3_shinkai_name::PyShinkaiName;

#[pyclass]
#[derive(Debug, Clone)]
pub struct PySerializedLLMProvider {
    pub inner: SerializedLLMProvider,
}

#[pymethods]
impl PySerializedLLMProvider {
    #[new]
    pub fn new(kwargs: Option<&PyDict>) -> PyResult<Self> {
        let full_identity_name = kwargs
            .and_then(|k| k.get_item("full_identity_name").ok().flatten())
            .and_then(|v| v.extract::<PyShinkaiName>().ok())
            .map(|py_name| py_name.inner)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("full_identity_name is required"))?;

        let id = kwargs
            .and_then(|k| k.get_item("id").ok().flatten())
            .and_then(|v| v.extract::<String>().ok())
            .unwrap_or_else(|| String::new());
        let perform_locally = kwargs
            .and_then(|k| k.get_item("perform_locally").ok().flatten())
            .and_then(|v| v.extract::<bool>().ok())
            .unwrap_or_else(|| false);
        let external_url = kwargs
            .and_then(|k| k.get_item("external_url").ok().flatten())
            .and_then(|v| v.extract::<String>().ok());
        let api_key = kwargs
            .and_then(|k| k.get_item("api_key").ok().flatten())
            .and_then(|v| v.extract::<String>().ok());
        let model = kwargs
            .and_then(|k| k.get_item("model").ok().flatten())
            .and_then(|v| v.extract::<PyAgentLLMInterface>().ok())
            .map(|py_model| py_model.inner)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("model is required"))?;
        let toolkit_permissions = kwargs
            .and_then(|k| k.get_item("toolkit_permissions").ok().flatten())
            .and_then(|v| v.extract::<Vec<String>>().ok())
            .unwrap_or_else(|| Vec::new());
        let storage_bucket_permissions = kwargs
            .and_then(|k| k.get_item("storage_bucket_permissions").ok().flatten())
            .and_then(|v| v.extract::<Vec<String>>().ok())
            .unwrap_or_else(|| Vec::new());
        let allowed_message_senders = kwargs
            .and_then(|k| k.get_item("allowed_message_senders").ok().flatten())
            .and_then(|v| v.extract::<Vec<String>>().ok())
            .unwrap_or_else(|| Vec::new());

        Ok(Self {
            inner: SerializedLLMProvider {
                id,
                full_identity_name,
                perform_locally,
                external_url,
                api_key,
                model,
                toolkit_permissions,
                storage_bucket_permissions,
                allowed_message_senders,
            },
        })
    }

    #[staticmethod]
    pub fn new_with_defaults(
        full_identity_name: String,
        id: String,
        external_url: String,
        model: String,
        api_key: Option<String>,
    ) -> PyResult<Self> {
        let full_identity_name =
            ShinkaiName::new(full_identity_name).map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))?;
        let model = AgentLLMInterface::from_str(&model)
            .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid model"))?;

        Ok(Self {
            inner: SerializedLLMProvider {
                id,
                full_identity_name,
                perform_locally: false,
                external_url: Some(external_url),
                api_key,
                model,
                toolkit_permissions: Vec::new(),
                storage_bucket_permissions: Vec::new(),
                allowed_message_senders: Vec::new(),
            },
        })
    }

    #[staticmethod]
    pub fn from_json_str(s: &str) -> PyResult<Self> {
        let inner: Result<SerializedLLMProvider, SerdeError> = serde_json::from_str(s);
        match inner {
            Ok(agent) => Ok(Self { inner: agent }),
            Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())),
        }
    }

    pub fn to_json_str(&self) -> PyResult<String> {
        let json_str: Result<String, SerdeError> = serde_json::to_string(&self.inner);
        match json_str {
            Ok(s) => Ok(s),
            Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())),
        }
    }
}
