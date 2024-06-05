use super::chain_detection_embeddings::{
    top_score_message_history_summary_embeddings, top_score_summarize_these_embeddings,
    top_score_summarize_this_embeddings,
};
use crate::agent::error::AgentError;
use crate::agent::execution::chains::inference_chain_router::InferenceChainDecision;
use crate::agent::execution::chains::inference_chain_trait::{
    InferenceChain, InferenceChainContext, InferenceChainResult, ScoreResult,
};
use crate::agent::execution::chains::summary_chain::chain_detection_embeddings::top_score_summarize_other_embeddings;
use crate::agent::execution::prompts::prompts::{JobPromptGenerator, SubPrompt};
use crate::agent::execution::user_message_parser::ParsedUserMessage;
use crate::agent::job::{Job, JobLike, JobStepResult};
use crate::agent::job_manager::JobManager;
use crate::db::ShinkaiDB;
use crate::vector_fs::vector_fs::VectorFS;
use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::stream::StreamExt;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::model_type::{
    EmbeddingModelType, OllamaTextEmbeddingsInference, TextEmbeddingsInference,
};
use shinkai_vector_resources::vector_resource::BaseVectorResource;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;

/// Inference Chain used for summarizing
#[derive(Debug, Clone)]
pub struct SummaryInferenceChain {
    pub context: InferenceChainContext,
    this_checked: ScoreResult,
    these_checked: ScoreResult,
    message_history_checked: ScoreResult,
}

impl SummaryInferenceChain {
    pub fn new(context: InferenceChainContext, score_results: HashMap<String, ScoreResult>) -> Self {
        let this_checked = score_results
            .get("this_check")
            .unwrap_or(&ScoreResult::new_empty())
            .clone();
        let these_checked = score_results
            .get("these_check")
            .unwrap_or(&ScoreResult::new_empty())
            .clone();
        let message_history_checked = score_results
            .get("message_history_check")
            .unwrap_or(&ScoreResult::new_empty())
            .clone();

        Self {
            context,
            this_checked,
            these_checked,
            message_history_checked,
        }
    }
}

#[async_trait]
impl InferenceChain for SummaryInferenceChain {
    fn chain_id() -> String {
        "summary_inference_chain".to_string()
    }

    fn chain_context(&mut self) -> &mut InferenceChainContext {
        &mut self.context
    }

    async fn run_chain(&mut self) -> Result<InferenceChainResult, AgentError> {
        let response = self
            .start_summary_inference_chain(
                self.context.db.clone(),
                self.context.vector_fs.clone(),
                self.context.full_job.clone(),
                self.context.user_message.clone(),
                self.context.agent.clone(),
                self.context.execution_context.clone(),
                self.context.generator.clone(),
                self.context.user_profile.clone(),
                self.context.max_iterations,
                self.context.max_tokens_in_prompt,
            )
            .await?;
        let job_execution_context = self.context.execution_context.clone();
        Ok(InferenceChainResult::new(response, job_execution_context))
    }
}

impl SummaryInferenceChain {
    /// An inference chain for summarizing every VR in the job's scope.
    #[async_recursion]
    #[instrument(skip(generator, vector_fs, db))]
    pub async fn start_summary_inference_chain(
        &self,
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        user_message: ParsedUserMessage,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        max_iterations: u64,
        max_tokens_in_prompt: usize,
    ) -> Result<String, AgentError> {
        // Perform the checks
        let checks = vec![
            self.this_checked.clone(),
            self.these_checked.clone(),
            self.message_history_checked.clone(),
        ];
        let highest_score_checked =
            checks
                .into_iter()
                .filter(|check| check.passed_scoring)
                .fold(ScoreResult::new_empty(), |acc, check| {
                    if check.score > acc.score {
                        check
                    } else {
                        acc
                    }
                });

        // Later implement this alternative summary flow
        // if message_history_check.1 == highest_score_check.1 {
        if self.these_checked.score == highest_score_checked.score
            || self.this_checked.score == highest_score_checked.score
        {
            Self::start_summarize_job_context_sub_chain(
                db,
                vector_fs,
                full_job,
                user_message,
                agent,
                execution_context,
                generator,
                user_profile,
                max_tokens_in_prompt,
            )
            .await
        } else {
            Self::start_summarize_job_context_sub_chain(
                db,
                vector_fs,
                full_job,
                user_message,
                agent,
                execution_context,
                generator,
                user_profile,
                max_tokens_in_prompt,
            )
            .await
        }
    }

    /// Core logic which summarizes VRs in the job context.
    async fn start_summarize_job_context_sub_chain(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        user_message: ParsedUserMessage,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        max_tokens_in_prompt: usize,
    ) -> Result<String, AgentError> {
        let scope = full_job.scope();
        let resource_count =
            JobManager::count_number_of_resources_in_job_scope(vector_fs.clone(), &user_profile, scope).await?;

        // Optimization TODO:
        // If a significant amount of VRs, simply search first to find the the top 5 most relevant and summarize them fully.
        // The rest summarize as 1-2 line sentences and list them up to 25.

        // Get a stream that retrieves all resources in the job scope automatically, and chunk it in groups of 5 (same as stream buffer size)
        let resource_stream =
            JobManager::retrieve_all_resources_in_job_scope_stream(vector_fs.clone(), scope, &user_profile).await;
        let mut chunks = resource_stream.chunks(5);

        // For each chunk parallelize creating a detailed summary for each
        let mut num_resources_processed = 0;
        let mut detailed_summaries = Vec::new();
        while let Some(resources) = chunks.next().await {
            let resource_count = resources.len();

            // Create a future for each resource in the chunk
            let futures = resources.into_iter().map(|resource| {
                Self::generate_detailed_summary_for_resource(
                    resource,
                    generator.clone(),
                    user_message.clone(),
                    agent.clone(),
                    max_tokens_in_prompt,
                    1,
                )
            });
            let results = futures::future::join_all(futures).await;

            // Handle each future's result individually
            for result in results {
                match result {
                    Ok(summary) => detailed_summaries.push(summary),
                    Err(e) => shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Error,
                        &format!("Error generating detailed summary: {}", e),
                    ),
                }
            }
            num_resources_processed += resource_count;
        }

        let joined_summaries = detailed_summaries
            .iter()
            .enumerate()
            .map(|(index, summary)| {
                if index < detailed_summaries.len() - 1 {
                    format!("{}\n\n---\n", summary)
                } else {
                    format!("{}\n\n", summary)
                }
            })
            .collect::<String>();

        Ok(joined_summaries)
    }

    #[async_recursion]
    pub async fn generate_detailed_summary_for_resource(
        resource: BaseVectorResource,
        generator: RemoteEmbeddingGenerator,
        user_message: ParsedUserMessage,
        agent: SerializedAgent,
        max_tokens_in_prompt: usize,
        attempt_count: u64,
    ) -> Result<String, AgentError> {
        let resource_sub_prompts = SubPrompt::convert_resource_into_subprompts(&resource, 97);

        // TODO: Make sure the whole document gets parsed into chunks that fit the LLMs max tokens minus some front buffer for the actual prompt
        // Split the list of resource_sub_prompts into chunks that fit in the max tokens in prompt
        // Implement a method on SubPrompt does this chunking and token counting
        // let sub_prompt_chunks = resource_sub_prompts.

        let resource_source = resource.as_trait_object().source();
        let prompt = JobPromptGenerator::summary_chain_detailed_summary_prompt(
            user_message.clone(),
            resource_sub_prompts,
            resource_source,
        );
        eprintln!("generate_detailed_summary_for_resource> Prompt: {:?}", prompt);

        // Extract the JSON from the inference response Result and proceed forward
        let response = JobManager::inference_agent_markdown(agent.clone(), prompt.clone()).await?;
        let answer = &JobManager::advanced_extract_key_from_inference_response(
            agent.clone(),
            response.clone(),
            prompt.clone(),
            vec![
                "answer".to_string(),
                "markdown".to_string(),
                "summary".to_string(),
                "text".to_string(),
            ],
            3,
        )
        .await?;

        // Split into chunks and do improved parsing
        let filtered_answer = answer.replace("\\n_", "\\n");
        let mut chunks: Vec<&str> = filtered_answer.split("\n\n").collect();
        if chunks.last().map_or(false, |last| last.trim().is_empty()) {
            chunks.pop();
        }
        let filtered_answer = if chunks.len() == 3 {
            let mut title = chunks[0].replace("Title:", "");
            let intro = chunks[1].replace("Summary:", "").replace("Intro:", "");
            let mut list = chunks[2].replace("List:", "");

            // Add the title tag if it doesnt exist
            if !title.is_empty() && !title.trim().starts_with('#') {
                title = format!("## {}", title);
            }

            // Filter the list for common pitfalls of dumb LLMs
            list = list
                .replace("[Content Title]:", "")
                .replace("[Bulletpoint Title]: ", "")
                .replace("Bulletpoint Description:", "-");

            format!("{}\n\n{}\n\n{}", title.trim(), intro.trim(), list.trim())
        } else {
            filtered_answer
        };

        if filtered_answer.len() < 100 && attempt_count < 2 {
            return Self::generate_detailed_summary_for_resource(
                resource,
                generator.clone(),
                user_message.clone(),
                agent.clone(),
                max_tokens_in_prompt,
                attempt_count + 1,
            )
            .await;
        }

        Ok(filtered_answer)
    }

    /// Validates that the message is relevant enough (from quick checking) to bother doing a wide amount of  embedding checking.
    async fn validate_user_message_first_pass(
        user_message: ParsedUserMessage,
        generator: RemoteEmbeddingGenerator,
        job_scope: &JobScope,
        step_history: &Vec<JobStepResult>,
    ) -> bool {
        // Temporary english-only approach, later use a few key embedding strings and a lower threshold as a 1st pass if relevant at all.
        let direct_substrings = vec![
            "summary",
            "sumry",
            "sunnary",
            "sunary",
            "summry",
            "summry",
            "summari",
            "sumery",
            "sumnary",
            "sumarry",
            "summarey",
            "sumary",
            "summarize",
            "sumrize",
            "sumrise",
            "summarise",
            "sumarise",
            "sumarize",
            "sunnarize",
            "sunrize",
            "sunarize",
            "sunarise",
            "recap",
            " re cap ",
            "overview",
            "over view",
            "overiew",
            "ovrview",
            "verview",
            "overvue",
            "overvew",
            "overew",
            "overbiew",
        ];
        let direct_other_substrings = ["explain", "what is", "what do", "what does", "detail", "rundown"];

        // Check if any of the direct substrings are in the user message
        let user_message_no_code_blocks = user_message.get_output_string_without_codeblocks();
        let substring_result = direct_substrings.iter().any(|substring| {
            user_message_no_code_blocks
                .to_lowercase()
                .contains(&substring.to_lowercase())
        });

        // If no summary substring and no previous message, check with "other" phrases to improve quality
        if !substring_result && step_history.is_empty() && !job_scope.is_empty() {
            // First do another substring check to keep things efficient/fast, before using the other embeddings
            let other_substring_result = direct_other_substrings.iter().any(|substring| {
                user_message_no_code_blocks
                    .to_lowercase()
                    .contains(&substring.to_lowercase())
            });
            if other_substring_result {
                if let Ok(other_check_result) = other_check(&generator, &user_message, job_scope).await {
                    if other_check_result.passed_scoring {
                        return true;
                    }
                }
            }
        }
        substring_result
    }

    /// Checks if the job's task asks to summarize in one of many ways.
    pub async fn validate_user_message_requests_summary(
        user_message: ParsedUserMessage,
        generator: RemoteEmbeddingGenerator,
        job_scope: &JobScope,
        step_history: &Vec<JobStepResult>,
    ) -> Option<InferenceChainDecision> {
        // If there are no VRs in the scope, we don't use the summary chain for now (may change when we do advanced message history summarization)
        if job_scope.is_empty() {
            return None;
        }

        // Do the quick first pass check
        let first_pass_result =
            Self::validate_user_message_first_pass(user_message.clone(), generator.clone(), job_scope, step_history)
                .await;
        if !first_pass_result {
            return None;
        }

        // Perform the vector search detailed checks.
        let these_check_result = these_check(&generator, &user_message, job_scope)
            .await
            .unwrap_or(ScoreResult::new_empty());
        let this_check_result = this_check(&generator, &user_message, job_scope, step_history)
            .await
            .unwrap_or(ScoreResult::new_empty());
        // For now we don't use the message history check as its just useless/inefficient to do the embeddings gen
        // Later on may be useful
        //let  message_history_check = message_history_check(&generator, &user_message)
        //     .await
        //     .unwrap_or(ScoreResult::new_empty());
        let message_history_check_result = ScoreResult::new_empty();

        /// Create the scores hashmap
        let mut scores = HashMap::new();
        scores.insert("this_check".to_string(), this_check_result);
        scores.insert("these_check".to_string(), these_check_result);
        scores.insert("message_history_check".to_string(), message_history_check_result);

        Some(InferenceChainDecision::new(Self::chain_id(), scores))
    }
}

/// Returns the passing score for the summary chain checks
fn passing_score(generator: &RemoteEmbeddingGenerator) -> f32 {
    match generator.model_type() {
        EmbeddingModelType::TextEmbeddingsInference(TextEmbeddingsInference::AllMiniLML6v2) => 0.68,
        EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::AllMiniLML6v2) => 0.68,
        EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M) => {
            0.85
        }
        _ => {
            eprintln!(
                "Embedding model type not accounted for in Summary Chain detection! Add: {:?}",
                generator.model_type()
            );
            0.75
        }
    }
}

/// Checks if the user message's similarity score passes for any of the "these" summary strings
async fn these_check(
    generator: &RemoteEmbeddingGenerator,
    user_message: &ParsedUserMessage,
    job_scope: &JobScope,
) -> Result<ScoreResult, AgentError> {
    // Get user message embedding, without code blocks for clarity in task
    let user_message_embedding = user_message
        .generate_embedding_filtered(generator.clone(), false, true)
        .await?;
    let passing = passing_score(&generator.clone());
    let these_score = top_score_summarize_these_embeddings(generator.clone(), &user_message_embedding).await?;
    // println!("Top These score: {:.2}", these_score);
    Ok(ScoreResult::new(
        these_score,
        these_score > passing && !job_scope.is_empty(),
    ))
}

/// Checks if the user message's similarity score passes for any of the "this" summary strings
async fn this_check(
    generator: &RemoteEmbeddingGenerator,
    user_message: &ParsedUserMessage,
    job_scope: &JobScope,
    step_history: &Vec<JobStepResult>,
) -> Result<ScoreResult, AgentError> {
    // Get user message embedding, without code blocks for clarity in task
    let user_message_embedding = user_message
        .generate_embedding_filtered(generator.clone(), false, true)
        .await?;

    let passing = passing_score(&generator.clone());
    let this_score = top_score_summarize_this_embeddings(generator.clone(), &user_message_embedding).await?;
    // println!("Top This score: {:.2}", this_score);

    // Get current job task code block count, and the previous job task's code block count if it exists in step history
    let current_code_block_count = user_message.get_code_block_elements().len();

    // Get current user message code block count, and the previous user message's code block count if it exists in step history
    let current_code_block_count = user_message.get_code_block_elements().len();
    let previous_code_block_count = step_history
        .last()
        .and_then(|step| {
            step.get_latest_user_message_parsed()
                .map(|message| message.get_code_block_elements().len())
        })
        .unwrap_or(0);

    // Check if the user message and the previous user message have code blocks
    let code_block_exists = current_code_block_count > 0 && previous_code_block_count > 0;
    // Only pass if there are VRs in scope, and no code blocks. This is to allow QA chain to deal with codeblock summary for now.
    let check = this_score > passing && !job_scope.is_empty() && !code_block_exists;

    Ok(ScoreResult::new(this_score, check))
}

/// Checks if the user message's similarity score passes for any of the other summary-esque strings (used only if no prev messages)
async fn other_check(
    generator: &RemoteEmbeddingGenerator,
    user_message: &ParsedUserMessage,
    job_scope: &JobScope,
) -> Result<ScoreResult, AgentError> {
    // Get user message embedding, without code blocks for clarity in task
    let user_message_embedding = user_message
        .generate_embedding_filtered(generator.clone(), false, true)
        .await?;
    let passing = passing_score(&generator.clone());
    let these_score = top_score_summarize_other_embeddings(generator.clone(), &user_message_embedding).await?;
    // println!("Top These score: {:.2}", these_score);
    Ok(ScoreResult::new(
        these_score,
        these_score > passing && !job_scope.is_empty(),
    ))
}

/// Checks if the user message's similarity score passes for the "message history" summary string
async fn message_history_check(
    generator: &RemoteEmbeddingGenerator,
    user_message: &ParsedUserMessage,
) -> Result<ScoreResult, AgentError> {
    // Get user message embedding, without code blocks for clarity in task
    let user_message_embedding = user_message
        .generate_embedding_filtered(generator.clone(), false, true)
        .await?;

    let passing = passing_score(&generator.clone());
    let message_history_score =
        top_score_message_history_summary_embeddings(generator.clone(), &user_message_embedding).await?;
    // println!("Top Message history score: {:.2}", message_history_score);
    Ok(ScoreResult::new(message_history_score, message_history_score > passing))
}
