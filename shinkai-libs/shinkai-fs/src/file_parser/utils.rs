use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub enum TextChunkingStrategy {
    /// The default text chunking strategy implemented in VR lib using local parsing.
    V1,
}