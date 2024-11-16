use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, ToSchema)]
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

#[derive(Debug, Clone, Deserialize, ToSchema)]
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
