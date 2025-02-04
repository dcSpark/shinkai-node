use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value as JsonValue;

#[derive(Copy, Debug, Clone, PartialEq)]
pub enum RunnerType {
    Any,
    OnlyHost,
    OnlyDocker,
}

impl Default for RunnerType {
    fn default() -> Self {
        RunnerType::Any
    }
}

impl RunnerType {
    fn as_str(&self) -> &'static str {
        match self {
            RunnerType::Any => "any",
            RunnerType::OnlyHost => "only_host",
            RunnerType::OnlyDocker => "only_docker",
        }
    }

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "any" => Ok(RunnerType::Any),
            "only_host" => Ok(RunnerType::OnlyHost),
            "only_docker" => Ok(RunnerType::OnlyDocker),
            _ => Err(format!("Invalid runner type: {}", s)),
        }
    }
}

impl Serialize for RunnerType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RunnerType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        RunnerType::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Copy, Debug, Clone, PartialEq)]
pub enum OperatingSystem {
    Linux,
    MacOS,
    Windows,
}

impl OperatingSystem {
    fn as_str(&self) -> &'static str {
        match self {
            OperatingSystem::Linux => "linux",
            OperatingSystem::MacOS => "macos",
            OperatingSystem::Windows => "windows",
        }
    }

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "linux" => Ok(OperatingSystem::Linux),
            "macos" => Ok(OperatingSystem::MacOS),
            "windows" => Ok(OperatingSystem::Windows),
            _ => Err(format!("Invalid operating system: {}", s)),
        }
    }
}

impl Serialize for OperatingSystem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for OperatingSystem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        OperatingSystem::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    pub r#type: String,
    pub properties: serde_json::Value,
    pub required: Vec<String>,
}

impl Serialize for ToolResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let helper = Helper {
            result_type: self.r#type.clone(),
            properties: self.properties.clone(),
            required: self.required.clone(),
        };

        helper.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ToolResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = Helper::deserialize(deserializer)?;

        Ok(ToolResult {
            r#type: helper.result_type,
            properties: helper.properties,
            required: helper.required,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Helper {
    #[serde(rename = "type", alias = "result_type")]
    result_type: String,
    properties: JsonValue,
    required: Vec<String>,
}

impl ToolResult {
    pub fn new(result_type: String, properties: serde_json::Value, required: Vec<String>) -> Self {
        ToolResult {
            r#type: result_type,
            properties,
            required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_runner_type_serialization() {
        let runner_types = vec![RunnerType::Any, RunnerType::OnlyHost, RunnerType::OnlyDocker];

        for runner_type in runner_types {
            let serialized = serde_json::to_string(&runner_type).unwrap();
            let deserialized: RunnerType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(runner_type, deserialized);
        }
    }

    #[test]
    fn test_operating_system_serialization() {
        let operating_systems = vec![OperatingSystem::Linux, OperatingSystem::MacOS, OperatingSystem::Windows];

        for os in operating_systems {
            let serialized = serde_json::to_string(&os).unwrap();
            let deserialized: OperatingSystem = serde_json::from_str(&serialized).unwrap();
            assert_eq!(os, deserialized);
        }
    }

    #[test]
    fn test_tool_result_serialization() {
        let tool_result = ToolResult {
            r#type: "test_type".to_string(),
            properties: json!({
                "key": "value",
                "number": 42
            }),
            required: vec!["key".to_string()],
        };

        let serialized = serde_json::to_string(&tool_result).unwrap();
        let deserialized: ToolResult = serde_json::from_str(&serialized).unwrap();

        assert_eq!(tool_result.r#type, deserialized.r#type);
        assert_eq!(tool_result.properties, deserialized.properties);
        assert_eq!(tool_result.required, deserialized.required);
    }

    #[test]
    fn test_invalid_runner_type() {
        let result = RunnerType::from_str("invalid");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid runner type: invalid");
    }

    #[test]
    fn test_invalid_operating_system() {
        let result = OperatingSystem::from_str("invalid");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid operating system: invalid");
    }

    #[test]
    fn test_helper_serialization() {
        let helper = Helper {
            result_type: "test_type".to_string(),
            properties: json!({
                "key": "value",
                "number": 42
            }),
            required: vec!["key".to_string()],
        };

        let serialized = serde_json::to_string(&helper).unwrap();
        let deserialized: Helper = serde_json::from_str(&serialized).unwrap();

        assert_eq!(helper.result_type, deserialized.result_type);
        assert_eq!(helper.properties, deserialized.properties);
        assert_eq!(helper.required, deserialized.required);
    }

    #[test]
    fn test_runner_type_from_json_string() {
        let json_str = r#""any""#;
        let deserialized: RunnerType = serde_json::from_str(json_str).unwrap();
        assert_eq!(deserialized, RunnerType::Any);

        let json_str = r#""only_host""#;
        let deserialized: RunnerType = serde_json::from_str(json_str).unwrap();
        assert_eq!(deserialized, RunnerType::OnlyHost);

        let json_str = r#""only_docker""#;
        let deserialized: RunnerType = serde_json::from_str(json_str).unwrap();
        assert_eq!(deserialized, RunnerType::OnlyDocker);
    }

    #[test]
    fn test_operating_system_from_json_string() {
        let json_str = r#""linux""#;
        let deserialized: OperatingSystem = serde_json::from_str(json_str).unwrap();
        assert_eq!(deserialized, OperatingSystem::Linux);

        let json_str = r#""macos""#;
        let deserialized: OperatingSystem = serde_json::from_str(json_str).unwrap();
        assert_eq!(deserialized, OperatingSystem::MacOS);

        let json_str = r#""windows""#;
        let deserialized: OperatingSystem = serde_json::from_str(json_str).unwrap();
        assert_eq!(deserialized, OperatingSystem::Windows);
    }

    #[test]
    fn test_tool_result_from_json_string() {
        let json_str = r#"{
            "type": "test_type",
            "properties": {
                "key": "value",
                "number": 42
            },
            "required": ["key"]
        }"#;

        let deserialized: ToolResult = serde_json::from_str(json_str).unwrap();
        assert_eq!(deserialized.r#type, "test_type");
        assert_eq!(
            deserialized.properties,
            json!({
                "key": "value",
                "number": 42
            })
        );
        assert_eq!(deserialized.required, vec!["key"]);
    }

    #[test]
    fn test_helper_from_json_string() {
        let json_str = r#"{
            "type": "test_type",
            "properties": {
                "key": "value",
                "number": 42
            },
            "required": ["key"]
        }"#;

        let deserialized: Helper = serde_json::from_str(json_str).unwrap();
        assert_eq!(deserialized.result_type, "test_type");
        assert_eq!(
            deserialized.properties,
            json!({
                "key": "value",
                "number": 42
            })
        );
        assert_eq!(deserialized.required, vec!["key"]);
    }
}
