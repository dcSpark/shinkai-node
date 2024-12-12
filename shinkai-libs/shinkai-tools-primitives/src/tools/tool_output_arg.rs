#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolOutputArg {
    pub json: String,
}

impl ToolOutputArg {
    pub fn empty() -> Self {
        Self { json: "".to_string() }
    }
}
