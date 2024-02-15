use pyo3::prelude::*;
use pyo3::types::PyString;
use pyo3::wrap_pyfunction;
use serde::Deserialize;
use serde::Serialize;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::shinkai_utils::job_scope::LocalScopeEntry;
use shinkai_message_primitives::shinkai_utils::job_scope::VectorFSScopeEntry;

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PyJobScope {
    pub inner: JobScope,
}

#[pymethods]
impl PyJobScope {
    #[new]
    pub fn new() -> Self {
        // TODO: Someday add args
        Self {
            inner: JobScope::new(Vec::new(), Vec::new()),
        }
    }

    #[staticmethod]
    pub fn new_empty() -> Self {
        Self {
            inner: JobScope::new(Vec::new(), Vec::new()),
        }
    }

    #[staticmethod]
    pub fn from_json_str(s: &str) -> PyResult<Self> {
        let inner =
            JobScope::from_json_str(s).map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        Ok(Self { inner })
    }

    pub fn to_json_str(&self) -> PyResult<String> {
        self.inner
            .to_json_str()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    }

    pub fn is_empty(&self) -> bool {
        self.inner.local.is_empty() && self.inner.vector_fs.is_empty()
    }
}
