#[derive(Debug, Clone)]
pub struct GlobalSearchContextBuilderParams {
    pub use_community_summary: bool,
    pub column_delimiter: String,
    pub shuffle_data: bool,
    pub include_community_rank: bool,
    pub min_community_rank: u32,
    pub community_rank_name: String,
    pub include_community_weight: bool,
    pub community_weight_name: String,
    pub normalize_community_weight: bool,
    pub max_tokens: usize,
    pub context_name: String,
    //conversation_history: Option<ConversationHistory>,
    // conversation_history_user_turns_only: bool,
    // conversation_history_max_turns: Option<i32>,
}

pub struct ConversationHistory {}
