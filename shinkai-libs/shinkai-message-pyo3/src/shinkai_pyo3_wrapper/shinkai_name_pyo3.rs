use pyo3::{PyResult, FromPyObject, types::PyString, PyAny};
use shinkai_message_primitives::schemas::inbox_name::{InboxName, InboxNameError};
use pyo3::prelude::*;

#[pyclass]
pub struct PyInboxName {
    pub inner: InboxName,
}

#[pymethods]
impl PyInboxName {
    #[new]
    fn new(name: &str) -> PyResult<Self> {
        match InboxName::new(name.to_string()) {
            Ok(inbox_name) => Ok(PyInboxName {
                inner: inbox_name,
            }),
            Err(err) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(err.to_string())),
        }
    }

    #[getter]
    fn get_inner(&self) -> PyResult<String> {
        Ok(self.inner.to_string())
    }

    #[setter]
    fn set_inner(&mut self, value: String) -> PyResult<()> {
        match InboxName::new(value) {
            Ok(inbox_name) => {
                self.inner = inbox_name;
                Ok(())
            },
            Err(err) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(err.to_string())),
        }
    }
}

#[pyclass]
pub struct PyInboxNameError {
    pub inner: InboxNameError,
}

impl From<PyInboxNameError> for PyErr {
    fn from(err: PyInboxNameError) -> Self {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(err.inner.to_string())
    }
}

impl<'source> FromPyObject<'source> for PyInboxName {
    fn extract(ob: &'source PyAny) -> PyResult<Self> {
        let s = ob.downcast::<PyString>()?.to_str()?.to_owned();
        PyInboxName::new(&s)
    }
}
