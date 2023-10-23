use pyo3::prelude::*;
use pyo3::types::PyDict;
use shinkai_message_primitives::schemas::agents::serialized_agent::AgentLLMInterface;
use shinkai_message_primitives::schemas::agents::serialized_agent::GenericAPI;
use shinkai_message_primitives::schemas::agents::serialized_agent::LocalLLM;
use shinkai_message_primitives::schemas::agents::serialized_agent::OpenAI;

#[pyclass]
#[derive(Debug, Clone)]
pub struct PyAgentLLMInterface {
    pub inner: AgentLLMInterface,
}

#[pymethods]
impl PyAgentLLMInterface {
    #[new]
    pub fn new(s: String) -> PyResult<Self> {
        if s.starts_with("openai:") {
            let model_type = s.strip_prefix("openai:").unwrap_or("").to_string();
            Ok(Self {
                inner: AgentLLMInterface::OpenAI(OpenAI { model_type }),
            })
        } else if s.starts_with("genericapi:") {
            let model_type = s.strip_prefix("genericapi:").unwrap_or("").to_string();
            Ok(Self {
                inner: AgentLLMInterface::GenericAPI(GenericAPI { model_type }),
            })
        } 
        else {
            Ok(Self {
                inner: AgentLLMInterface::LocalLLM(LocalLLM {}),
            })
        }
    }

    #[staticmethod]
    pub fn new_openai(model_type: String) -> Self {
        let open_ai = OpenAI {
            model_type,
        };
        Self {
            inner: AgentLLMInterface::OpenAI(open_ai),
        }
    }

    #[staticmethod]
    pub fn new_genericapi(model_type: String) -> Self {
        let generic_api = GenericAPI {
            model_type,
        };
        Self {
            inner: AgentLLMInterface::GenericAPI(generic_api),
        }
    }

    #[staticmethod]
    pub fn new_localllm() -> Self {
        Self {
            inner: AgentLLMInterface::LocalLLM(LocalLLM {}),
        }
    }

    pub fn get_model(&self) -> PyResult<String> {
        match &self.inner {
            AgentLLMInterface::OpenAI(open_ai) => Ok(format!("openai:{}", open_ai.model_type)),
            AgentLLMInterface::GenericAPI(generic_ai) => Ok(format!("genericapi:{}", generic_ai.model_type)),
            AgentLLMInterface::LocalLLM(_) => Ok("LocalLLM".to_string()),
        }
    }
}