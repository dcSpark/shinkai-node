use arrow_schema::{DataType, Field, Schema};
use shinkai_vector_resources::{model_type::EmbeddingModelType, resource_errors::VRError};
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
    /// - vector: Fixed-size list of 32-bit floats (nullable)
    ///
    /// The vector field's size is determined by the embedding model's dimensions.
    pub fn create_schema(embedding_model: &EmbeddingModelType) -> Result<Arc<Schema>, VRError> {
        let vector_dimensions = embedding_model.vector_dimensions()?;

        Ok(Arc::new(Schema::new(vec![
            Field::new(Self::name_field(), DataType::Utf8, false),
            Field::new(Self::is_system_field(), DataType::Boolean, false),
            Field::new(Self::is_enabled_field(), DataType::Boolean, false),
            Field::new(Self::version_field(), DataType::Utf8, false),
            Field::new(Self::prompt_field(), DataType::Utf8, false),
            Field::new(Self::is_favorite_field(), DataType::Boolean, false),
            Field::new(
                Self::vector_field(),
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    vector_dimensions.try_into().unwrap(),
                ),
                true,
            ),
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

    pub fn vector_field() -> &'static str {
        "vector"
    }
}
