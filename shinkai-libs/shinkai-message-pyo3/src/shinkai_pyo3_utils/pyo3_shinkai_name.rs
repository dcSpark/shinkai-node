use pyo3::prelude::*;
use pyo3::types::PyString;
use pyo3::wrap_pyfunction;
use serde_json::Error as SerdeError;
use shinkai_message_primitives::schemas::agents::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

#[pyclass]
#[derive(Debug, Clone)]
pub struct PyShinkaiName {
    pub inner: ShinkaiName,
}

#[pymethods]
impl PyShinkaiName {
    #[new]
    pub fn new(raw_name: String) -> PyResult<Self> {
        match ShinkaiName::new(raw_name) {
            Ok(inner) => Ok(Self { inner }),
            Err(err) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(err)),
        }
    }

    #[staticmethod]
    pub fn from_node_name(node_name: String) -> PyResult<Self> {
        match ShinkaiName::from_node_name(node_name) {
            Ok(inner) => Ok(Self { inner }),
            Err(err) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(err.to_string())),
        }
    }

    #[staticmethod]
    pub fn from_node_and_profile_names(node_name: String, profile_name: String) -> PyResult<Self> {
        match ShinkaiName::from_node_and_profile_names(node_name, profile_name) {
            Ok(inner) => Ok(Self { inner }),
            Err(err) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(err)),
        }
    }

    #[getter]
    pub fn get_full_name(&self) -> PyResult<String> {
        Ok(self.inner.full_name.clone())
    }

    #[getter]
    pub fn get_node_name_string(&self) -> PyResult<String> {
        Ok(self.inner.node_name.clone())
    }

    #[getter]
    pub fn get_profile_name_string(&self) -> PyResult<Option<String>> {
        Ok(self.inner.profile_name.clone())
    }

    #[getter]
    pub fn get_subidentity_type(&self) -> PyResult<Option<String>> {
        Ok(self.inner.subidentity_type.clone().map(|t| t.to_string()))
    }

    #[getter]
    pub fn get_subidentity_name(&self) -> PyResult<Option<String>> {
        Ok(self.inner.subidentity_name.clone())
    }
}
