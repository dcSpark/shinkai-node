use pyo3::prelude::*;
use pyo3::types::PyDict;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::AgentLLMInterface;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::GenericAPI;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::Groq;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LocalLLM;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::Ollama;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::OpenAI;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::ShinkaiBackend;

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
        } else if s.starts_with("ollama:") {
            let model_type = s.strip_prefix("ollama:").unwrap_or("").to_string();
            Ok(Self {
                inner: AgentLLMInterface::Ollama(Ollama { model_type }),
            })
        } else if s.starts_with("shinkai-backend:") {
            let model_type = s.strip_prefix("shinkai-backend:").unwrap_or("").to_string();
            Ok(Self {
                inner: AgentLLMInterface::ShinkaiBackend(ShinkaiBackend { model_type }),
            })
        } else if s.starts_with("groq:") {
            let model_type = s.strip_prefix("groq:").unwrap_or("").to_string();
            Ok(Self {
                inner: AgentLLMInterface::Groq(Groq { model_type }),
            })
        } else {
            Ok(Self {
                inner: AgentLLMInterface::LocalLLM(LocalLLM {}),
            })
        }
    }

    #[staticmethod]
    pub fn new_openai(model_type: String) -> Self {
        let open_ai = OpenAI { model_type };
        Self {
            inner: AgentLLMInterface::OpenAI(open_ai),
        }
    }

    #[staticmethod]
    pub fn new_genericapi(model_type: String) -> Self {
        let generic_api = GenericAPI { model_type };
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
            AgentLLMInterface::Ollama(ollama) => Ok(format!("ollama:{}", ollama.model_type)),
            AgentLLMInterface::Groq(groq) => Ok(format!("groq:{}", groq.model_type)),
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => {
                Ok(format!("shinkai-backend:{}", shinkai_backend.model_type()))
            }
            AgentLLMInterface::LocalLLM(_) => Ok("LocalLLM".to_string()),
        }
    }
}
