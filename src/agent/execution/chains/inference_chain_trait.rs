use crate::agent::execution::user_message_parser::ParsedUserMessage;
use crate::agent::{error::AgentError, job::Job};
use crate::db::ShinkaiDB;
use crate::vector_fs::vector_fs::VectorFS;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use std::{collections::HashMap, sync::Arc};

/// Trait that abstracts top level functionality between the inference chains. This allows
/// the inference chain router to work with them all easily.
#[async_trait]
pub trait InferenceChain {
    // fn new(context: InferenceChainContext) -> Self;
    // async fn start_chain() -> Result<String, AgentError>;
}

/// Struct that represents the generalized context available to all chains as input. Note not all chains require
/// using all fields in this struct, but they are available nonetheless.
pub struct InferenceChainContext {
    db: Arc<ShinkaiDB>,
    vector_fs: Arc<VectorFS>,
    full_job: Job,
    user_message: ParsedUserMessage,
    agent: SerializedAgent,
    execution_context: HashMap<String, String>,
    generator: RemoteEmbeddingGenerator,
    user_profile: ShinkaiName,
    max_iterations: u64,
    iteration_count: u64,
    max_tokens_in_prompt: usize,
    score_results: HashMap<String, ScoreResult>,
}

impl InferenceChainContext {
    pub fn new(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        user_message: ParsedUserMessage,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        max_iterations: u64,
        iteration_count: u64,
        max_tokens_in_prompt: usize,
        score_results: HashMap<String, ScoreResult>,
    ) -> Self {
        Self {
            db,
            vector_fs,
            full_job,
            user_message,
            agent,
            execution_context,
            generator,
            user_profile,
            max_iterations,
            iteration_count,
            max_tokens_in_prompt,
            score_results,
        }
    }
}

// The result from scoring an inference chain (checking if its the right chain to route to)
pub struct ScoreResult {
    pub score: f32,
    pub passed_scoring: bool,
}
impl ScoreResult {
    pub fn new(score: f32, passed_scoring: bool) -> Self {
        Self { score, passed_scoring }
    }
}
