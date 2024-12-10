use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DynamicToolType {
    DenoDynamic,
    PythonDynamic,
}

impl std::fmt::Display for DynamicToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DynamicToolType::DenoDynamic => write!(f, "deno_dynamic"),
            DynamicToolType::PythonDynamic => write!(f, "python_dynamic"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub enum CodeLanguage {
    Typescript,
    Python,
}

impl std::fmt::Display for CodeLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeLanguage::Typescript => write!(f, "typescript"),
            CodeLanguage::Python => write!(f, "python"),
        }
    }
}

impl CodeLanguage {
    pub fn to_dynamic_tool_type(&self) -> Option<DynamicToolType> {
        match self {
            CodeLanguage::Typescript => Some(DynamicToolType::DenoDynamic),
            CodeLanguage::Python => Some(DynamicToolType::PythonDynamic),
        }
    }
}
