use async_trait::async_trait;
// use polars::prelude::*;
use std::collections::HashMap;

// TODO: Serialize and Deserialize polars::frame::DataFrame
type DataFrame = Vec<u8>;

#[async_trait]
pub trait GlobalContextBuilder {
    /// Build the context for the global search mode.
    async fn build_context(
        &self,
        conversation_history: Option<ConversationHistory>,
        context_builder_params: Option<HashMap<String, serde_json::Value>>,
    ) -> (Vec<String>, HashMap<String, DataFrame>);
}

pub struct ConversationHistory {}
