use pyo3::{exceptions::PyValueError, prelude::*, types::PyDict};
use shinkai_message_primitives::{
    shinkai_message::shinkai_message::{
        EncryptedShinkaiBody, ExternalMetadata, MessageBody, ShinkaiBody, ShinkaiMessage, ShinkaiVersion, InternalMetadata,
    },
    shinkai_utils::encryption::EncryptionMethod,
};

use crate::shinkai_pyo3_utils::{pyo3_job_scope::PyJobScope, pyo3_agent_llm_interface::PyAgentLLMInterface, pyo3_serialized_agent::PySerializedAgent, pyo3_shinkai_name::PyShinkaiName};
use super::{shinkai_builder_pyo3::PyShinkaiMessageBuilder, encryption_method_pyo3::PyEncryptionMethod, message_schema_type_pyo3::PyMessageSchemaType};

#[pymodule]
fn shinkai_message_pyo3(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyShinkaiMessage>()?;
    m.add_class::<PyExternalMetadata>()?;
    m.add_class::<PyInternalMetadata>()?;
    m.add_class::<PyShinkaiMessageBuilder>()?;
    m.add_class::<PyEncryptionMethod>()?;
    m.add_class::<PyJobScope>()?;
    m.add_class::<PyAgentLLMInterface>()?;
    m.add_class::<PySerializedAgent>()?;
    m.add_class::<PyShinkaiName>()?;
    m.add_class::<PyMessageSchemaType>()?;
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
