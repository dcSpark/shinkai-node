use pyo3::prelude::*;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::Exo;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::Gemini;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::Groq;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LocalLLM;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::Ollama;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::OpenAI;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::OpenRouter;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::ShinkaiBackend;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::TogetherAI;

#[pyclass]
#[derive(Debug, Clone)]
pub struct PyLLMProviderInterface {
    pub inner: LLMProviderInterface,
}

#[pymethods]
impl PyLLMProviderInterface {
    #[new]
    pub fn new(s: String) -> PyResult<Self> {
        if s.starts_with("openai:") {
            let model_type = s.strip_prefix("openai:").unwrap_or("").to_string();
            Ok(Self {
                inner: LLMProviderInterface::OpenAI(OpenAI { model_type }),
            })
        } else if s.starts_with("togetherai:") {
            let model_type = s.strip_prefix("togetherai:").unwrap_or("").to_string();
            Ok(Self {
                inner: LLMProviderInterface::TogetherAI(TogetherAI { model_type }),
            })
        } else if s.starts_with("ollama:") {
            let model_type = s.strip_prefix("ollama:").unwrap_or("").to_string();
            Ok(Self {
                inner: LLMProviderInterface::Ollama(Ollama { model_type }),
            })
        } else if s.starts_with("shinkai-backend:") {
            let model_type = s.strip_prefix("shinkai-backend:").unwrap_or("").to_string();
            Ok(Self {
                inner: LLMProviderInterface::ShinkaiBackend(ShinkaiBackend { model_type }),
            })
        } else if s.starts_with("groq:") {
            let model_type = s.strip_prefix("groq:").unwrap_or("").to_string();
            Ok(Self {
                inner: LLMProviderInterface::Groq(Groq { model_type }),
            })
        } else if s.starts_with("gemini:") {
            let model_type = s.strip_prefix("gemini:").unwrap_or("").to_string();
            Ok(Self {
                inner: LLMProviderInterface::Gemini(Gemini { model_type }),
            })
        } else if s.starts_with("exo:") {
            let model_type = s.strip_prefix("exo:").unwrap_or("").to_string();
            Ok(Self {
                inner: LLMProviderInterface::Exo(Exo { model_type }),
            })
        } else if s.starts_with("openrouter:") {
            let model_type = s.strip_prefix("openrouter:").unwrap_or("").to_string();
            Ok(Self {
                inner: LLMProviderInterface::OpenRouter(OpenRouter { model_type }),
            })
        } else {
            Ok(Self {
                inner: LLMProviderInterface::LocalLLM(LocalLLM {}),
            })
        }
    }

    #[staticmethod]
    pub fn new_openai(model_type: String) -> Self {
        let open_ai = OpenAI { model_type };
        Self {
            inner: LLMProviderInterface::OpenAI(open_ai),
        }
    }

    #[staticmethod]
    pub fn new_togetherai(model_type: String) -> Self {
        let generic_api = TogetherAI { model_type };
        Self {
            inner: LLMProviderInterface::TogetherAI(generic_api),
        }
    }

    #[staticmethod]
    pub fn new_openrouter(model_type: String) -> Self {
        let open_router = OpenRouter { model_type };
        Self {
            inner: LLMProviderInterface::OpenRouter(open_router),
        }
    }

    #[staticmethod]
    pub fn new_localllm() -> Self {
        Self {
            inner: LLMProviderInterface::LocalLLM(LocalLLM {}),
        }
    }

    pub fn get_model(&self) -> PyResult<String> {
        match &self.inner {
            LLMProviderInterface::OpenAI(open_ai) => Ok(format!("openai:{}", open_ai.model_type)),
            LLMProviderInterface::TogetherAI(generic_ai) => Ok(format!("togetherai:{}", generic_ai.model_type)),
            LLMProviderInterface::Ollama(ollama) => Ok(format!("ollama:{}", ollama.model_type)),
            LLMProviderInterface::Groq(groq) => Ok(format!("groq:{}", groq.model_type)),
            LLMProviderInterface::Gemini(gemini) => Ok(format!("gemini:{}", gemini.model_type)),
            LLMProviderInterface::Exo(exo) => Ok(format!("exo:{}", exo.model_type)),
            LLMProviderInterface::OpenRouter(open_router) => Ok(format!("openrouter:{}", open_router.model_type)),
            LLMProviderInterface::Claude(claude) => Ok(format!("claude:{}", claude.model_type)),
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => {
                Ok(format!("shinkai-backend:{}", shinkai_backend.model_type()))
            }
            LLMProviderInterface::LocalLLM(_) => Ok("LocalLLM".to_string()),
        }
    }
}
