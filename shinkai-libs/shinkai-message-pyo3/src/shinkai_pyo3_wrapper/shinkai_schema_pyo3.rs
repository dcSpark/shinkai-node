use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use serde_json::Result;
use shinkai_message_primitives::{shinkai_message::shinkai_message_schemas::{JobCreationInfo}, schemas::inbox_name::InboxName, shinkai_utils::job_scope::{JobScope, LocalScopeEntry, DBScopeEntry}};

use super::shinkai_name_pyo3::PyInboxName;

#[pyclass]
#[derive(Clone)]
pub struct PyJobScope {
    pub inner: JobScope,
}

#[pymethods]
impl PyJobScope {
    // #[new]
    // #[args(local = "None", database = "None")]
    // fn new(local: Option<Vec<LocalScopeEntry>>, database: Option<Vec<DBScopeEntry>>) -> Self {
    //     PyJobScope {
    //         inner: JobScope {
    //             local: local.unwrap_or_else(Vec::new),
    //             database: database.unwrap_or_else(Vec::new),
    //         },
    //     }
    // }

    // fn to_bytes(&self) -> PyResult<Vec<u8>> {
    //     let j = serde_json::to_string(&self.inner).map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    //     Ok(j.into_bytes())
    // }

    // #[staticmethod]
    // fn from_bytes(bytes: &[u8]) -> PyResult<Self> {
    //     let job_scope = serde_json::from_slice(bytes).map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    //     Ok(PyJobScope { inner: job_scope })
    // }

    // #[staticmethod]
    // fn from_json_str(s: &str) -> PyResult<Self> {
    //     let job_scope = serde_json::from_str(s).map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    //     Ok(PyJobScope { inner: job_scope })
    // }

    // fn to_json_str(&self) -> PyResult<String> {
    //     let json_str = serde_json::to_string(&self.inner).map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    //     Ok(json_str)
    // }
}

#[pyclass]
pub struct PyJobCreation {
    pub inner: JobCreationInfo,
}
