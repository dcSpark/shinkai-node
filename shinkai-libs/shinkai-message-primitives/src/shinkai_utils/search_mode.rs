use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub enum VectorSearchMode {
    FillUpTo25k,
    MergeSiblings,
}
