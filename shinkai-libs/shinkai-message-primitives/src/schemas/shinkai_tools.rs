use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ToolType {
    Deno,
    DenoDynamic,
    Python,
    PythonDynamic,
    Network,
    Internal,
}

impl std::fmt::Display for ToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolType::Deno => write!(f, "Deno"),
            ToolType::DenoDynamic => write!(f, "deno_dynamic"),
            ToolType::Python => write!(f, "Python"),
            ToolType::PythonDynamic => write!(f, "python_dynamic"),
            ToolType::Network => write!(f, "Network"),
            ToolType::Internal => write!(f, "Internal"),
        }
    }
}

#[derive(Deserialize, ToSchema, Clone)]
pub enum Language {
    Typescript,
    Python,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Typescript => write!(f, "typescript"),
            Language::Python => write!(f, "python"),
        }
    }
}
