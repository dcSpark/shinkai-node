use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use pyo3::types::PyString;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;

#[pyclass]
#[derive(Debug, Clone)]
pub struct PyMessageSchemaType {
    pub inner: MessageSchemaType,
}

#[pymethods]
impl PyMessageSchemaType {
    #[new]
    pub fn new(schema_type: String) -> PyResult<Self> {
        let inner = MessageSchemaType::from_str(&schema_type)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid schema type"))?;
        Ok(Self { inner })
    }

    pub fn to_str(&self) -> PyResult<&'static str> {
        Ok(self.inner.to_str())
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[pymodule]
fn shinkai_message_schemas(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyMessageSchemaType>()?;
    Ok(())
}