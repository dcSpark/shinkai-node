use arrow_schema::{DataType, Field, Schema};
use shinkai_vector_resources::resource_errors::VRError;
use std::sync::Arc;

pub struct ShinkaiPromptSchema;

impl ShinkaiPromptSchema {
    /// Creates a new Schema for Shinkai prompts with the following fields:
    /// - name: UTF-8 string (non-nullable)
    /// - is_system: Boolean (non-nullable)
    /// - is_enabled: Boolean (non-nullable)
    /// - version: UTF-8 string (non-nullable, starting from "1")
    /// - prompt: UTF-8 string (non-nullable)
    /// - is_favorite: Boolean (non-nullable)
    pub fn create_schema() -> Result<Arc<Schema>, VRError> {
        Ok(Arc::new(Schema::new(vec![
            Field::new(Self::name_field(), DataType::Utf8, false),
            Field::new(Self::is_system_field(), DataType::Boolean, false),
            Field::new(Self::is_enabled_field(), DataType::Boolean, false),
            Field::new(Self::version_field(), DataType::Utf8, false),
            Field::new(Self::prompt_field(), DataType::Utf8, false),
            Field::new(Self::is_favorite_field(), DataType::Boolean, false),
        ])))
    }

    pub fn name_field() -> &'static str {
        "name"
    }

    pub fn is_system_field() -> &'static str {
        "is_system"
    }

    pub fn is_enabled_field() -> &'static str {
        "is_enabled"
    }

    pub fn version_field() -> &'static str {
        "version"
    }

    pub fn prompt_field() -> &'static str {
        "prompt"
    }

    pub fn is_favorite_field() -> &'static str {
        "is_favorite"
    }
}