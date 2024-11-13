#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolArgument {
    pub name: String,
    pub arg_type: String,
    pub description: String,
    pub is_required: bool,
}

impl ToolArgument {
    /// Creates a new ToolArgument
    pub fn new(name: String, arg_type: String, description: String, is_required: bool) -> Self {
        Self {
            name,
            arg_type,
            description,
            is_required,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolOutputArg {
    pub json: String,
}
