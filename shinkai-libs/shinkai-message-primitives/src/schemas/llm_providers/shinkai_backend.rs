use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct QuotaResponse {
    pub has_quota: bool,
    pub tokens_quota: u64,
    pub used_tokens: u64,
    pub reset_time: u64,
}
