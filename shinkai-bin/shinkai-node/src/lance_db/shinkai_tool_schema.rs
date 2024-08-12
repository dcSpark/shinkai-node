use arrow_schema::{DataType, Field, Schema};
use shinkai_vector_resources::{model_type::EmbeddingModelType, resource_errors::VRError};
use std::sync::Arc;

pub struct ShinkaiToolSchema;

impl ShinkaiToolSchema {
    /// Creates a new Schema for Shinkai tools with the following fields:
    /// - tool_key: UTF-8 string (non-nullable)
    /// - vector: Fixed-size list of 32-bit floats (nullable)
    /// - tool_data: UTF-8 string (non-nullable)
    /// - tool_type: UTF-8 string (non-nullable)
    /// - tool_header: UTF-8 string (non-nullable)
    /// - is_enabled: Boolean (non-nullable)
    /// - config: UTF-8 string (nullable)
    ///
    /// The vector field's size is determined by the embedding model's dimensions.
    pub fn create_schema(embedding_model: &EmbeddingModelType) -> Result<Arc<Schema>, VRError> {
        let vector_dimensions = embedding_model.vector_dimensions()?;

        Ok(Arc::new(Schema::new(vec![
            Field::new(Self::tool_key_field(), DataType::Utf8, false),
            Field::new(
                Self::vector_field(),
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    vector_dimensions.try_into().unwrap(),
                ),
                true,
            ),
            Field::new(Self::tool_type_field(), DataType::Utf8, false),
            Field::new(Self::tool_data_field(), DataType::Utf8, false),
            Field::new(Self::tool_header_field(), DataType::Utf8, false),
            Field::new(Self::is_enabled_field(), DataType::Boolean, false),
            Field::new(Self::config_field(), DataType::Utf8, true),
        ])))
    }

    pub fn tool_key_field() -> &'static str {
        "tool_key"
    }

    pub fn vector_field() -> &'static str {
        "vector"
    }

    pub fn tool_data_field() -> &'static str {
        "tool_data"
    }

    pub fn tool_type_field() -> &'static str {
        "tool_type"
    }

    pub fn tool_header_field() -> &'static str {
        "tool_header"
    }

    pub fn is_enabled_field() -> &'static str {
        "is_enabled"
    }

    pub fn config_field() -> &'static str {
        "config"
    }

    pub fn vector_dimensions(embedding_model: &EmbeddingModelType) -> Result<usize, VRError> {
        embedding_model.vector_dimensions()
    }
}
