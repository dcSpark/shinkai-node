use crate::agent::error::AgentError;
use crate::agent::execution::prompts::prompts::{JobPromptGenerator, SubPrompt};
use crate::agent::execution::user_message_parser::ParsedUserMessage;
use crate::agent::job::{Job, JobId, JobLike, JobStepResult};
use crate::agent::job_manager::JobManager;
use crate::db::ShinkaiDB;
use crate::vector_fs::vector_fs::VectorFS;
use async_recursion::async_recursion;
use futures::stream::StreamExt;
use keyphrases::KeyPhraseExtractor;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::model_type::EmbeddingModelType;
use shinkai_vector_resources::vector_resource::BaseVectorResource;
use shinkai_vector_resources::vector_resource::BaseVectorResource;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;

use super::chain_detection_embeddings::{
    top_score_message_history_summary_embeddings, top_score_summarize_these_embeddings,
    top_score_summarize_this_embeddings,
};

impl JobManager {
    /// An inference chain for summarizing every VR in the job's scope.
    #[async_recursion]
    #[instrument(skip(generator, vector_fs, db))]
    pub async fn start_summary_inference_chain(
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
        let this_check = this_check(&generator, &user_message, full_job.scope(), &full_job.step_history).await?;
        let these_check = these_check(&generator, &user_message, full_job.scope()).await?;
        let message_history_check = message_history_check(&generator, &user_message).await?;

        let checks = vec![this_check, these_check, message_history_check];
        let highest_score_check = checks
            .into_iter()
            .filter(|check| check.0)
            .fold((false, 0.0f32), |acc, check| if check.1 > acc.1 { check } else { acc });

        // Later implement this alternative summary flow
        // if message_history_check.1 == highest_score_check.1 {
        if these_check.1 == highest_score_check.1 || this_check.1 == highest_score_check.1 {
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
            JobManager::retrieve_all_resources_in_job_scope_stream(vector_fs.clone(), &scope, &user_profile).await;
        let mut chunks = resource_stream.chunks(5);

        // For each chunk parallelize creating a detailed summary for each
        let mut num_resources_processed = 0;
        let mut detailed_summaries = Vec::new();
        while let Some(resources) = chunks.next().await {
            println!("Received chunk of resources: {}", resources.len());
            let resource_count = resources.len();

            // Create a future for each resource in the chunk
            let futures = resources.into_iter().map(|resource| {
                Self::generate_detailed_summary_for_resource(
                    resource,
                    generator.clone(),
                    user_message.clone(),
                    agent.clone(),
                    max_tokens_in_prompt,
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
            .map(|summary| format!("{}\n\n\n", summary))
            .collect::<String>();

        Ok(joined_summaries)
    }

    pub async fn generate_detailed_summary_for_resource(
        resource: BaseVectorResource,
        generator: RemoteEmbeddingGenerator,
        user_message: ParsedUserMessage,
        agent: SerializedAgent,
        max_tokens_in_prompt: usize,
    ) -> Result<String, AgentError> {
        let resource_sub_prompts = SubPrompt::convert_resource_into_subprompts(&resource, 97);

        // TODO: Make sure the whole document gets parsed into chunks that fit the LLMs max tokens minus some front buffer for the actual prompt
        // Split the list of resource_sub_prompts into chunks that fit in the max tokens in prompt
        // Implement a method on SubPrompt does this chunking and token counting
        // let sub_prompt_chunks = resource_sub_prompts.

        let resource_source = resource.as_trait_object().source();
        let prompt = JobPromptGenerator::summary_chain_detailed_summary_prompt(
            user_message,
            resource_sub_prompts,
            resource_source,
        );

        // Specify the keys that we need to extract from the LLM response
        let mut potential_keys = HashMap::new();
        potential_keys.insert("title", vec!["name", "answer", "markdown"]);
        potential_keys.insert("intro", vec!["introduction", "text", "paragraph", "explanation"]);
        potential_keys.insert(
            "list",
            vec![
                "bullets",
                "points",
                "bulletpoints",
                "bulletpoint",
                "lists",
                "bullet-points",
            ],
        );

        // Extract the JSON from the inference response Result and proceed forward
        let response = JobManager::inference_agent_json(agent.clone(), prompt.clone()).await?;
        let extracted_keys_map = JobManager::advanced_extract_multi_keys_from_inference_response(
            agent.clone(),
            response,
            prompt.clone(),
            potential_keys.clone(),
            3,
        )
        .await?;

        // Now parse the extracted keys into the output markdown summary string
        let mut summary = String::new();
        if let Some(title) = extracted_keys_map.get("title") {
            if !title.is_empty() && !title.trim().starts_with("#") {
                summary += "#";
            }
            eprintln!("Adding title: {}", title);
            summary += &format!(" {}\n\n", title.trim());
        }
        if let Some(intro) = extracted_keys_map.get("intro") {
            eprintln!("Adding intro: {}", intro);
            summary += &format!("{}\n\n", intro);
        }
        if let Some(list) = extracted_keys_map.get("list") {
            eprintln!("Adding list: {}", list);
            summary += &format!("{}\n", list);
        }

        Ok(summary)
    }

    // TODO: Optimization. Directly check if the text holds any substring of summary/summarize/recap botched or not. If yes, only then do the embedding checks.
    /// Checks if the job's task asks to summarize in one of many ways using vector search.
    pub async fn validate_user_message_requests_summary(
        user_message: ParsedUserMessage,
        generator: RemoteEmbeddingGenerator,
        job_scope: &JobScope,
        step_history: &Vec<JobStepResult>,
    ) -> bool {
        // Perform the checks
        let these_check = these_check(&generator, &user_message, job_scope)
            .await
            .unwrap_or((false, 0.0));
        let this_check = this_check(&generator, &user_message, job_scope, step_history)
            .await
            .unwrap_or((false, 0.0));
        let message_history_check = message_history_check(&generator, &user_message)
            .await
            .unwrap_or((false, 0.0));

        // Check if any of the conditions passed
        these_check.0 || this_check.0 || message_history_check.0
    }
}

/// Returns the passing score for the summary chain checks
fn passing_score(generator: &RemoteEmbeddingGenerator) -> f32 {
    if generator.model_type()
        == EmbeddingModelType::TextEmbeddingsInference(
            shinkai_vector_resources::model_type::TextEmbeddingsInference::AllMiniLML6v2,
        )
    {
        0.68
    } else {
        eprintln!(
            "Embedding model type not accounted for in Summary Chain detection! Add: {:?}",
            generator.model_type()
        );
        0.75
    }
}

/// Checks if the user message's similarity score passes for any of the "these" summary strings
async fn these_check(
    generator: &RemoteEmbeddingGenerator,
    user_message: &ParsedUserMessage,
    job_scope: &JobScope,
) -> Result<(bool, f32), AgentError> {
    // Get user message embedding, without code blocks for clarity in task
    let user_message_embedding = user_message
        .generate_embedding_filtered(generator.clone(), false, true)
        .await?;
    let passing = passing_score(&generator.clone());
    let these_score = top_score_summarize_these_embeddings(generator.clone(), &user_message_embedding).await?;
    println!("Top These score: {:.2}", these_score);
    Ok((these_score > passing && !job_scope.is_empty(), these_score))
}

/// Checks if the user message's similarity score passes for any of the "this" summary strings
async fn this_check(
    generator: &RemoteEmbeddingGenerator,
    user_message: &ParsedUserMessage,
    job_scope: &JobScope,
    step_history: &Vec<JobStepResult>,
) -> Result<(bool, f32), AgentError> {
    // Get user message embedding, without code blocks for clarity in task
    let user_message_embedding = user_message
        .generate_embedding_filtered(generator.clone(), false, true)
        .await?;

    let passing = passing_score(&generator.clone());
    let this_score = top_score_summarize_this_embeddings(generator.clone(), &user_message_embedding).await?;
    println!("Top This score: {:.2}", this_score);

    // Get current job task code block count, and the previous job task's code block count if it exists in step history
    let current_code_block_count = job_task.get_elements_filtered(true, false).len();

    for step in step_history {
        println!("Step: {:?}", step);
    }

    // let previous_job_task = step_history.last().map(|step| step.).unwrap_or_default();
    // let previous_code_block_count = previous_job_task.get_elements_filtered(true, false).len();

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

    Ok((check, this_score))
}

/// Checks if the user message's similarity score passes for the "message history" summary string
async fn message_history_check(
    generator: &RemoteEmbeddingGenerator,
    user_message: &ParsedUserMessage,
) -> Result<(bool, f32), AgentError> {
    // Get user message embedding, without code blocks for clarity in task
    let user_message_embedding = user_message
        .generate_embedding_filtered(generator.clone(), false, true)
        .await?;

    let passing = passing_score(&generator.clone());
    let message_history_score =
        top_score_message_history_summary_embeddings(generator.clone(), &user_message_embedding).await?;
    println!("Top Message history score: {:.2}", message_history_score);
    Ok((message_history_score > passing, message_history_score))
}
