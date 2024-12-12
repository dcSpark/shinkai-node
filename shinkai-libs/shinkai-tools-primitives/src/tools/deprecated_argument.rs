// ... existing code ...

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DeprecatedArgument {
    pub name: String,
    pub arg_type: String,
    pub description: String,
    pub is_required: bool,
}

impl DeprecatedArgument {
    /// Creates a new DeprecatedArgument
    pub fn new(name: String, arg_type: String, description: String, is_required: bool) -> Self {
        Self {
            name,
            arg_type,
            description,
            is_required,
        }
    }
}
