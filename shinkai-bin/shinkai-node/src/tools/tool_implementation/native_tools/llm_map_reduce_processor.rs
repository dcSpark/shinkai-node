use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::parameters::{Parameters, Property};
use shinkai_tools_primitives::tools::{
    error::ToolError, shinkai_tool::ShinkaiToolHeader, tool_output_arg::ToolOutputArg,
};
use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;

use ed25519_dalek::SigningKey;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use tokio::sync::Mutex;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::managers::IdentityManager;
use crate::tools::tool_generation::v2_create_and_send_job_message;
use crate::{llm_provider::job_manager::JobManager, network::Node};

use async_trait::async_trait;
use tokio::time::{sleep, Duration};

use crate::tools::tool_implementation::tool_traits::ToolExecutor;

pub struct LlmMapReduceProcessorTool {
    pub tool: ShinkaiToolHeader,
}

impl LlmMapReduceProcessorTool {
    pub fn new() -> Self {
        Self {
            tool: ShinkaiToolHeader {
                name: "Shinkai LLM Map Reduce Processor".to_string(),
                description: r#"Tool for applying a prompt over a long text (longer than the context window of the LLM) using an AI LLM. 
This can be used to process complex requests, text analysis, text matching, text generation, and any other AI LLM task. over long texts."#
                    .to_string(),
                tool_router_key: "local:::__official_shinkai:::shinkai_llm_map_reduce_processor".to_string(),
                tool_type: "Rust".to_string(),
                formatted_tool_summary_for_ui: "Tool for processing prompts with LLM".to_string(),
                author: "@@official.shinkai".to_string(),
                version: "1.0".to_string(),
                enabled: true,
                input_args: {
                    let mut params = Parameters::new();
                    params.add_property("prompt".to_string(), "string".to_string(), "The prompt to apply over the data".to_string(), true);
                    params.add_property("data".to_string(), "string".to_string(), "The data to process".to_string(), true);
                    
                    // Add the optional tools array parameter
                    let tools_property = Property::with_array_items(
                        "List of tools names or tool router keys to be used with the prompt".to_string(),
                        Property::new("string".to_string(), "Tool".to_string())
                    );
                    params.properties.insert("tools".to_string(), tools_property);
                    
                    params
                },
                output_arg: ToolOutputArg {
                    json: r#"{"type": "object", "properties": {"response": {"type": "string"}}}"#.to_string(),
                },
                config: None,
                usage_type: None,
                tool_offering: None,
            },
        }
    }
}

async fn apply_prompt_over_fragment(prompt: String, bearer: String, llm_provider: String, db: Arc<SqliteManager>, 
    node_name: ShinkaiName, identity_manager: Arc<Mutex<IdentityManager>>, job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey, encryption_public_key: EncryptionPublicKey, signing_secret_key: SigningKey) -> Result<String, ToolError> {
        
    
    let response = v2_create_and_send_job_message(
        bearer.clone(),
        JobCreationInfo {
            scope: MinimalJobScope::default(),
            is_hidden: Some(true),
            associated_ui: None,
        },
        llm_provider,
        prompt,
        None,
        db.clone(),
        node_name,
        identity_manager,
        job_manager,
        encryption_secret_key,
        encryption_public_key,
        signing_secret_key,
    )
    .await
    .map_err(|_| ToolError::ExecutionError("Failed to create job".to_string()))?;
    let (res_sender, res_receiver) = async_channel::bounded(1);
    let inbox_name = InboxName::get_job_inbox_name_from_params(response.clone())
    .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(60 * 5); // 5 minutes timeout
    let delay = Duration::from_secs(1); // 1 second delay between polls

    let x = loop {
        let _ = Node::v2_get_last_messages_from_inbox_with_branches(
            db.clone(),
            bearer.clone(),
            inbox_name.to_string(),
            100,
            None,
            res_sender.clone(),
        )
        .await;

        let x = res_receiver
            .recv()
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?
            .map_err(|_| ToolError::ExecutionError("Failed to get messages".to_string()))?;

        if x.len() >= 2 {
            break x;
        }

        if start_time.elapsed() >= timeout {
            return Err(ToolError::ExecutionError("Timeout waiting for messages".to_string()));
        }

        sleep(delay).await;
    };
    let v_chat_message = match x.last() {
        Some(x) => x,
        None => return Err(ToolError::ExecutionError("No messages".to_string())),
    };
    let chat_message = match v_chat_message.last() {
        Some(chat_message) => chat_message,
        None => return Err(ToolError::ExecutionError("No messages".to_string())),
    };

    return Ok(chat_message.job_message.content.clone());
}

fn get_model_context_size(llm_provider: String) -> usize {
    // TODO This will be implemented in the future. Do not edit.
    match llm_provider.as_str() {
        "openai" => 16384,
        "anthropic" => 16384,
        "gemini" => 16384,
        "claude" => 16384,
        "ollama" => 16384,
        "llama" => 16384,
        "mistral" => 16384,
        "qwen" => 16384,
        "llama3" => 16384,
        "llama3.1" => 16384,
        _ => 16384,
    }
}

fn get_context_size_for_fragment(data: String) -> usize {
    // TODO This will be implemented in the future. Do not edit.
    return data.len();
}

fn split_text_into_chunks(text: &str, max_context_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let target_chunk_size = (max_context_size as f64 * 0.8) as usize; // 80% of max context
    let overlap_size = (max_context_size as f64 * 0.05) as usize; // 5% overlap
    
    let mut current_chunk = String::new();
    let mut words = text.split_whitespace().peekable();
    
    while let Some(word) = words.next() {
        let potential_chunk = if current_chunk.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current_chunk, word)
        };
        
        // Check if adding this word would exceed target size
        if get_context_size_for_fragment(potential_chunk.clone()) > target_chunk_size {
            // Save current chunk
            if !current_chunk.is_empty() {
                // Grab some words from the next section for overlap
                let mut overlap = String::new();
                let mut overlap_words = Vec::new();
                let mut peek_iter = words.clone();
                
                while get_context_size_for_fragment(overlap.clone()) < overlap_size {
                    if let Some(next_word) = peek_iter.next() {
                        if !overlap.is_empty() {
                            overlap.push(' ');
                        }
                        overlap.push_str(next_word);
                        overlap_words.push(next_word);
                    } else {
                        break;
                    }
                }
                
                // Add the overlap to the current chunk
                if !overlap.is_empty() {
                    if !current_chunk.is_empty() {
                        current_chunk.push(' ');
                    }
                    current_chunk.push_str(&overlap);
                }
                
                chunks.push(current_chunk);
                
                // Start new chunk with the current word and any overlap words
                current_chunk = word.to_string();
                // Don't advance the iterator for overlap words since we want to process them fully
                // in the next chunk
            }
        } else {
            current_chunk = potential_chunk;
        }
    }
    
    // Add the final chunk if it's not empty
    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }
    
    chunks
}

#[async_trait]
impl ToolExecutor for LlmMapReduceProcessorTool {
    async fn execute(
        bearer: String,
        _tool_id: String,
        _app_id: String,
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        signing_secret_key: SigningKey,
        parameters: &Map<String, Value>,
        llm_provider: String,
    ) -> Result<Value, ToolError> {
        // Extract the user request and the long text.
        let prompt = parameters
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let data = parameters
            .get("data")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Get the model's maximum context window size.
        let max_window = get_model_context_size(llm_provider.clone());

        // Split the long text into fragments within the model's context window.
        let chunks = split_text_into_chunks(&data, max_window);

        // --- Map Stage ---
        let mut map_results = Vec::new();
        for chunk in chunks {

                    // For each fragment, generate structured data using a mapping prompt.
        let map_prompt = format!(
            r#"
<intro>
    * The instructions are given in 4 sections: "intro", "fragment", "query", "rules", defined in tags: <tag></tag>.
    * Given a text "fragment" and a "query", extract relevant information following the specific "rules".
    * The text "fragment" is a part of a long text.
    * The "query" is the user request.
</intro>

<fragment>
{}
</fragment>

<query>
{}
</query>

<rules>
    * Analize and extract relevant information.
    * Write the reponse in a JSON Array format.
    * The JSON Array format is exactly: 
    {{ 
        "extracted_information": "string", 
        "rationale": "string", 
        "answer": "string", 
        "confidence_score": 0 | 1 | 2 | 3 | 4 | 5
    }}[]
    * The confidence score is a number between 0 [LOWEST] and 5 [HIGHEST].
    * Return the JSON Array only, nothing else, skip comments, ideas, strategies, etc.
</rules>
            "#,
            
                        chunk, 
                        prompt
                    );

            let map_result = apply_prompt_over_fragment(
                map_prompt.clone(),
                bearer.clone(),
                llm_provider.clone(),
                db.clone(),
                node_name.clone(),
                identity_manager.clone(),
                job_manager.clone(),
                encryption_secret_key.clone(),
                encryption_public_key.clone(),
                signing_secret_key.clone(),
            )
            .await?;
            map_results.push(map_result);
        }

        // Aggregate all the mapped results.
        let mut aggregated_maps = map_results.join("\n");

        // --- Collapse Stage ---
        // If the aggregated text exceeds the model's context window, collapse iteratively.

        while get_context_size_for_fragment(aggregated_maps.clone()) > max_window {

            let collapse_prompt = format!(
                r#"
<intro>
    * There are 4 sections: intro, "mapped_results", "query", "rules", defined in tags: <tag></tag>.
    * Given the following "mapped_results": each in JSON format:
    {{ 
        "extracted_information": "string", 
        "rationale": "string", 
        "answer": "string", 
        "confidence_score": 0 | 1 | 2 | 3 | 4 | 5
    }}[]
    * Compress and combine them into a single coherent JSON summary preserving essential details to answer the "query" following the "rules".
    * "query" is the user request.
    * "mapped_results" is a list of previous answers to the query.
</intro>
<mapped_results>
    {}
</mapped_results>
<query>
    {}
</query>
<rules>
    * Analize and extract relevant information.
    * Write the reponse in a JSON Array format.
    * The JSON Array format is exactly: 
    {{ 
        "extracted_information": "string", 
        "rationale": "string", 
        "answer": "string", 
        "confidence_score": 0 | 1 | 2 | 3 | 4 | 5
    }}[]
    * The confidence score is a number between 0 [LOWEST] and 5 [HIGHEST].
    * Return the JSON Array only, nothing else, skip comments, ideas, strategies, etc.
</rules>
"#,
                aggregated_maps.clone(),
                prompt.clone()
            );

            aggregated_maps = apply_prompt_over_fragment(
                collapse_prompt.clone(),
                bearer.clone(),
                llm_provider.clone(),
                db.clone(),
                node_name.clone(),
                identity_manager.clone(),
                job_manager.clone(),
                encryption_secret_key.clone(),
                encryption_public_key.clone(),
                signing_secret_key.clone(),
            )
            .await?;
        }

        // --- Reduce Stage ---
        // Use the collapsed result to generate the final answer.
        let reduce_prompt = format!(r#"
            Based on the following compressed mapped results (in JSON format), provide the final answer to the query: "{}".
            Return only the final answer in a JSON object with key "response". {}
            "#,
            prompt,
            aggregated_maps,
        );
        let final_result = apply_prompt_over_fragment(
            reduce_prompt,
            bearer.clone(),
            llm_provider.clone(),
            db.clone(),
            node_name.clone(),
            identity_manager.clone(),
            job_manager.clone(),
            encryption_secret_key.clone(),
            encryption_public_key.clone(),
            signing_secret_key.clone(),
        )
        .await?;

        Ok(json!({
            "response": final_result
        }))
    }
}
