use crate::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use serde::{Deserialize, Serialize};

// Based on the great job by crewai (mostly for for compatibility) https://docs.crewai.com

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CustomizedAgent {
    pub role: String,
    pub goal: String,
    pub backstory: String,
    pub llm: Option<SerializedLLMProvider>,
    pub tools: Vec<String>,
    pub function_calling_llm: Option<SerializedLLMProvider>,
    pub max_iter: Option<u32>,
    pub max_rpm: Option<u32>,
    pub max_execution_time: Option<u32>,
    pub verbose: bool,
    pub allow_delegation: bool,
    pub step_callback: Option<String>,
}

impl CustomizedAgent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        role: String,
        goal: String,
        backstory: String,
        llm: Option<SerializedLLMProvider>,
        tools: Vec<String>,
        function_calling_llm: Option<SerializedLLMProvider>,
        max_iter: Option<u32>,
        max_rpm: Option<u32>,
        max_execution_time: Option<u32>,
        verbose: bool,
        allow_delegation: bool,
        step_callback: Option<String>,
    ) -> Self {
        CustomizedAgent {
            role,
            goal,
            backstory,
            llm,
            tools,
            function_calling_llm,
            max_iter,
            max_rpm,
            max_execution_time,
            verbose,
            allow_delegation,
            step_callback,
        }
    }
}