use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DynamicToolType {
    DenoDynamic,
    PythonDynamic,
    AgentDynamic,
}

impl std::fmt::Display for DynamicToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DynamicToolType::DenoDynamic => write!(f, "deno_dynamic"),
            DynamicToolType::PythonDynamic => write!(f, "python_dynamic"),
            DynamicToolType::AgentDynamic => write!(f, "agent_dynamic"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CodeLanguage {
    #[serde(alias = "Typescript", alias = "TYPESCRIPT")]
    Typescript,
    #[serde(alias = "Python", alias = "PYTHON")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_language_serialization() {
        // Test positive cases
        let typescript = CodeLanguage::Typescript;
        let python = CodeLanguage::Python;

        // Serialize
        let typescript_str = serde_json::to_string(&typescript).unwrap();
        let python_str = serde_json::to_string(&python).unwrap();

        assert_eq!(typescript_str, "\"typescript\"");
        assert_eq!(python_str, "\"python\"");

        // Deserialize
        let typescript_deserialized: CodeLanguage = serde_json::from_str(&typescript_str).unwrap();
        let python_deserialized: CodeLanguage = serde_json::from_str(&python_str).unwrap();

        assert_eq!(typescript_deserialized, CodeLanguage::Typescript);
        assert_eq!(python_deserialized, CodeLanguage::Python);

        // Test case variations
        let case_variations = vec![
            ("\"typescript\"", CodeLanguage::Typescript),
            ("\"Typescript\"", CodeLanguage::Typescript),
            ("\"TYPESCRIPT\"", CodeLanguage::Typescript),
            ("\"python\"", CodeLanguage::Python),
            ("\"Python\"", CodeLanguage::Python),
            ("\"PYTHON\"", CodeLanguage::Python),
        ];

        for (input, expected) in case_variations {
            let result: CodeLanguage = serde_json::from_str(input).unwrap();
            assert_eq!(result, expected, "Failed to deserialize: {}", input);
        }

        // Test negative cases
        let invalid_cases = vec!["\"invalid\"", "TypeScript", "123", "null", "\"\""];

        for invalid_case in invalid_cases {
            let result = serde_json::from_str::<CodeLanguage>(invalid_case);
            assert!(result.is_err(), "Should fail to deserialize: {}", invalid_case);
        }
    }
}
