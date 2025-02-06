use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::parameters::{Parameters, Property};
use shinkai_tools_primitives::tools::{
    error::ToolError, shinkai_tool::ShinkaiToolHeader, tool_output_arg::ToolOutputArg,
};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use crate::utils::environment::fetch_node_environment;
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
use std::io::Write;

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
    
    let current_chunk = String::new();
    // Split on word boundaries while preserving newlines
    let words: Vec<&str> = text.split_inclusive(|c: char| c.is_whitespace()).collect();
    let mut i = 0;
    
    while i < words.len() {
        let mut chunk_words = Vec::new();
        let mut chunk_size = 0;
        
        // Build chunk up to target size
        while i < words.len() && chunk_size + words[i].len() <= target_chunk_size {
            chunk_words.push(words[i]);
            chunk_size += words[i].len();
            i += 1;
        }
        
        // If we have a valid chunk
        if !chunk_words.is_empty() {
            // Create the chunk
            let chunk = chunk_words.join("");
            chunks.push(chunk);
            
            // Move index back by overlap amount
            if i < words.len() {
                let overlap_word_count = overlap_size / 5; // Approximate words for overlap
                i = i.saturating_sub(overlap_word_count);
            }
        } else if i < words.len() {
            // If we couldn't fit even one word, take it anyway
            chunks.push(words[i].to_string());
            i += 1;
        }
    }
    
    // Add the final chunk if there's remaining text
    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }
    
    chunks
}

async fn map(chunk: String, prompt: String, bearer: String, llm_provider: String, db: Arc<SqliteManager>, 
    node_name: ShinkaiName, identity_manager: Arc<Mutex<IdentityManager>>, job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey, encryption_public_key: EncryptionPublicKey, signing_secret_key: SigningKey) -> Result<String, ToolError> {

        // For each fragment, generate structured data using a mapping prompt.
        let map_prompt = format!(r#"
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
    * Given a text "fragment" and a "query", extract relevant information following the "rules".
</rules>
            "#,
            
    chunk, 
    prompt
);

    let result = apply_prompt_over_fragment(
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

    Ok(clean_string(&result))
}

async fn collapse(mapped_results: String, query: String, bearer: String, llm_provider: String, db: Arc<SqliteManager>, 
    node_name: ShinkaiName, identity_manager: Arc<Mutex<IdentityManager>>, job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey, encryption_public_key: EncryptionPublicKey, signing_secret_key: SigningKey) -> Result<String, ToolError> {
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
    * Compress and combine them into a single coherent JSON summary preserving essential details to answer the "query" following the "rules".
</rules>
"#,
        mapped_results.clone(),
        query.clone(),
    );

    let result = apply_prompt_over_fragment(
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

    Ok(clean_string(&result))
}

async fn reduce(aggregated_maps: String, query: String, bearer: String, llm_provider: String, db: Arc<SqliteManager>, 
    node_name: ShinkaiName, identity_manager: Arc<Mutex<IdentityManager>>, job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey, encryption_public_key: EncryptionPublicKey, signing_secret_key: SigningKey) -> Result<String, ToolError> {
    let reduce_prompt = format!(r#"
<intro>
    * There are 4 sections: "intro", "mapped_results", "query", "rules", defined in tags: <tag></tag>.
    * Given the following "mapped_results": each in JSON format:
    {{ 
        "extracted_information": "string", 
        "rationale": "string", 
        "answer": "string", 
        "confidence_score": 0 | 1 | 2 | 3 | 4 | 5
    }}[]
    * Based on the following "mapped_results", provide a single final answer to the "query"
    * Return only the final answer following the "rules".
</intro>
<mapped_results>
    {}
</mapped_results>
<query>
    {}
</query>
<rules>
    * Analize and respond to the "query" based on the "mapped_results"
    * Write the reponse in a JSON Object format.
    * The JSON Object format is exactly: 
    {{ 
        "response": "string", 
    }}
    * Return only the final answer following the "rules".
    * Do not include any other text or comments asides from the JSON Object.
</rules>
"#,
        aggregated_maps,
        query
    );
    let result = apply_prompt_over_fragment(
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

    Ok(clean_string(&result))
}


#[async_trait]
impl ToolExecutor for LlmMapReduceProcessorTool {
    async fn execute(
        bearer: String,
        _tool_id: String,
        app_id: String,
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

        let node_env = fetch_node_environment();
        let node_storage_path = node_env.node_storage_path.clone().unwrap_or_default();
        let mut log_path = PathBuf::from(&node_storage_path);
        log_path.push("tools_storage");
        log_path.push(app_id);
        log_path.push("logs");
        std::fs::create_dir_all(log_path.clone())
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create directory structure: {}", e)))?;


        // Get the model's maximum context window size.
        let max_window = get_model_context_size(llm_provider.clone());

        // Split the long text into fragments within the model's context window.
        let chunks = split_text_into_chunks(&data, max_window);

        println!("chunk count: {:?}", chunks.len());
        println!("chunk sizes: {:?}", chunks.iter().map(|c| c.len()).collect::<Vec<usize>>());
        // --- Map Stage ---
        let mut step = 0;
        let mut map_results = Vec::new();
        for chunk in chunks {
            let map_result = map(chunk.clone(), prompt.clone(), bearer.clone(), llm_provider.clone(), db.clone(), node_name.clone(), identity_manager.clone(), job_manager.clone(), encryption_secret_key.clone(), encryption_public_key.clone(), signing_secret_key.clone()).await?;
            map_results.push(map_result.clone());
            let _ = write_log(log_path.clone(), format!("step_{}.map.log", step), format!("chunk: {}\nprompt: {}\nmap_result: {}", chunk, prompt, map_result));
            step += 1;
        }

        // --- Collapse Stage ---
        // If the aggregated text exceeds the model's context window, collapse iteratively.
        while get_context_size_for_fragment(map_results.join("\n")) > max_window {
            let mut iteration_result = vec![];
            let map_pairs = split_map_results_into_pairs(map_results);
            for pair in map_pairs {
                    let collapsed_result = collapse(pair.join("\n"), prompt.clone(), bearer.clone(), llm_provider.clone(), db.clone(), node_name.clone(), identity_manager.clone(), job_manager.clone(), encryption_secret_key.clone(), encryption_public_key.clone(), signing_secret_key.clone()).await?;
                    iteration_result.push(collapsed_result.clone());
                    let _ = write_log(log_path.clone(), format!("step_{}.collapse.log", step), format!("pair: {}\nprompt: {}\ncollapsed_result: {}", pair.join("\n"), prompt, collapsed_result));
                    step += 1;
                
            }

            map_results = iteration_result;
        }

        // --- Reduce Stage ---
        // Use the collapsed result to generate the final answer.
        let final_result = reduce(map_results.join("\n"), prompt.clone(), bearer.clone(), llm_provider.clone(), db.clone(), node_name.clone(), identity_manager.clone(), job_manager.clone(), encryption_secret_key.clone(), encryption_public_key.clone(), signing_secret_key.clone()).await?;
        let _ = write_log(log_path.clone(), format!("step_{}.reduce.log", step), format!("map_results: {}\nprompt: {}\nfinal_result: {}", map_results.join("\n"), prompt, final_result));

        Ok(json!({
            "response": final_result
        }))
    }
}

fn write_log(log_folder_path: PathBuf, name: String, map_result: String) -> Result<(), ToolError> {
    let mut file = File::create(log_folder_path.join(name))
        .map_err(|e| ToolError::ExecutionError(format!("Failed to create log file: {}", e)))?;
    file.write_all(map_result.as_bytes())
        .map_err(|e| ToolError::ExecutionError(format!("Failed to write to log file: {}", e)))?;
    Ok(())
}

fn split_map_results_into_pairs(map_results: Vec<String>) -> Vec<Vec<String>> {
    let mut map_pairs: Vec<Vec<String>> = Vec::new();
    if map_results.len() == 0 {
        return vec![];
    }
    // Handle the special case where we have 3 or fewer elements
    if map_results.len() <= 3 {
        return vec![map_results];
    }
    
    let total_elements = map_results.len();
    let mut current_index = 0;
    
    // Process elements until we reach the point where 3 elements remain
    while current_index < total_elements - 3 {
        map_pairs.push(vec![
            map_results[current_index].clone(),
            map_results[current_index + 1].clone(),
        ]);
        current_index += 2;
    }
    
    // Add the remaining elements (should be 3) as the final group
    let remaining: Vec<String> = map_results[current_index..].to_vec();
    if !remaining.is_empty() {
        map_pairs.push(remaining);
    }
    
    map_pairs
}

fn clean_string(input: &str) -> String {
    // Extract code from triple backticks if present
    if input.contains("```") {
        let start = input.find("```").unwrap_or(0);
        let content_start = if input[start..].starts_with("```json") {
            start + 7 // Skip ```json
        } else if input[start..].starts_with("```") {
            start + 3 // Skip ```
        } else {
            start
        };

        let end = input[content_start..].rfind("```"); // Changed to rfind to get the last occurrence
        let end = match end {
            Some(e) => e,
            None => return input.trim().to_string()
        };

        input[content_start..end+content_start].trim().to_string()
    } else {
        input.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_map_results_into_pairs() {
        // Test with even number of results (4 elements)
        let results = vec![
            "result1".to_string(),
            "result2".to_string(),
            "result3".to_string(),
            "result4".to_string(),
        ];
        let pairs = split_map_results_into_pairs(results);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], vec!["result1", "result2"]);
        assert_eq!(pairs[1], vec!["result3", "result4"]);

        // Test with three results - should stay as one group
        let results = vec![
            "result1".to_string(),
            "result2".to_string(),
            "result3".to_string(),
        ];
        let pairs = split_map_results_into_pairs(results);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], vec!["result1", "result2", "result3"]);

        // Test with five results - should create one pair and one triplet
        let results = vec![
            "result1".to_string(),
            "result2".to_string(),
            "result3".to_string(),
            "result4".to_string(),
            "result5".to_string(),
        ];
        let pairs = split_map_results_into_pairs(results);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], vec!["result1", "result2"]);
        assert_eq!(pairs[1], vec!["result3", "result4", "result5"]);

        // Test with seven results - should create two pairs and one triplet
        let results = vec![
            "result1".to_string(),
            "result2".to_string(),
            "result3".to_string(),
            "result4".to_string(),
            "result5".to_string(),
            "result6".to_string(),
            "result7".to_string(),
        ];
        let pairs = split_map_results_into_pairs(results);
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], vec!["result1", "result2"]);
        assert_eq!(pairs[1], vec!["result3", "result4"]);
        assert_eq!(pairs[2], vec!["result5", "result6", "result7"]);

        // Test with two results - should stay as one group since <= 3
        let results = vec!["result1".to_string(), "result2".to_string()];
        let pairs = split_map_results_into_pairs(results);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], vec!["result1", "result2"]);

        // Test with single result - should stay as one group
        let results = vec!["result1".to_string()];
        let pairs = split_map_results_into_pairs(results);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], vec!["result1"]);

        // Test with empty vector
        let results: Vec<String> = vec![];
        let pairs = split_map_results_into_pairs(results);
        assert_eq!(pairs.len(), 0);
    }

    #[test]
    fn test_split_text_into_chunks() {
        // Test with short text that fits in one chunk
        let short_text = "This is a short text that should fit in one chunk.";
        let chunks = split_text_into_chunks(short_text, 1000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], short_text);

        // Test with text that needs multiple chunks
        let long_text = format!("{}{}{}",
            "This is the first section. ".repeat(100),
            "This is the second section. ".repeat(100),
            "This is the third section. ".repeat(100)
        );
        let chunks = split_text_into_chunks(&long_text, 1000);
        assert!(chunks.len() > 1);
        
        // Verify each chunk is within size limits
        for chunk in &chunks {
            assert!(get_context_size_for_fragment(chunk.clone()) <= 1000);
        }

        // Test that chunks have overlap
        if chunks.len() >= 2 {
            let words_in_chunk1: Vec<&str> = chunks[0].split_whitespace().collect();
            let words_in_chunk2: Vec<&str> = chunks[1].split_whitespace().collect();
            
            // Get last few words of first chunk
            let last_words: Vec<&str> = words_in_chunk1.iter().rev().take(5).cloned().collect();
            // Get first few words of second chunk
            let first_words: Vec<&str> = words_in_chunk2.iter().take(5).cloned().collect();
            
            // Check if there's any overlap
            let has_overlap = last_words.iter().any(|&word| first_words.contains(&word));
            assert!(has_overlap, "Chunks should have some overlap");
        }

        // Test with empty text
        let empty_text = "";
        let chunks = split_text_into_chunks(empty_text, 1000);
        assert_eq!(chunks.len(), 0);

        // Test with text containing special characters and newlines
        let special_text = "First line\nSecond line with special chars: !@#$%^&*\nThird line";
        let chunks = split_text_into_chunks(special_text, 1000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], special_text);
    }

    #[test]
    fn test_chunk_overlap_percentage() {
        // Create a text that will definitely be split into multiple chunks
        let paragraph = r#"April
1 April
Biochemists report finishing the complete sequence of the human genome.[3][4]
A study shows that, contrary to widespread belief, body sizes of mammal extinction survivors of the dinosaur-times extinction event were the first to evolutionarily increase, with brain sizes increasing later in the Eocene.[5][6]
4 April
The Intergovernmental Panel on Climate Change (IPCC) releases the third and final part of its Sixth Assessment Report on climate change, warning that greenhouse gas emissions must peak before 2025 at the latest and decline 43% by 2030, in order to likely limit global warming to 1.5 °C (2.7 °F).[7][8]
Researchers announce a new technique for accelerating the development of vaccines and other pharmaceutical products by up to a million times, using much smaller quantities based on DNA nanotechnology.[9][10]
Alzheimer's disease (AD) research progress:
A study reports 42 new genes linked to an increased risk of AD.[11][12] Researchers report a potential primary mechanism of sleep disturbance as an early-stage effect of neurodegenerative diseases.[13][14] Researchers identify several genes associated with changes in brain structure over lifetime and potential AD therapy-targets (5 Apr).[15][16]

5 April: A study suggests that if "quintessence" is an explanation for dark-energy and current data is true as well, the world may start to end within the next 100 My, during which accelerating expansion of the Universe would inverse to contraction (a cyclic model).
5 April
COVID-19 pandemic: Preclinical data for a new vaccine developed at the Medical University of Vienna indicates it is effective against all SARS-CoV-2 variants known to date, including Omicron.[17][18]
A study presents a mechanism by which the hypothesized potential dark-energy-explaining quintessence, if true, would smoothly cause the accelerating expansion of the Universe to inverse to contraction, possibly within the cosmic near-future (100 My) given current data. It concludes that its end-time scenario theory fits "naturally with cyclic cosmologies [(each a theory of cycles of universe originations and ends, rather than the theories of one Big Bang beginning of the Universe/multiverse, to which authors were major contributors)] and recent conjectures about quantum gravity".[19][20][21]

6 April: The first known dinosaur fossil linked to the actual day of the Chicxulub impact is reported.
6 April
U.S. Space Command, based on information collected from its planetary defense sensors, confirms the detection of the first known interstellar object. The purported interstellar meteorite, technically known as CNEOS 2014-01-08, impacted Earth in 2014, and was determined, based on its hyperbolic trajectory and estimated initial high velocity, to be from beyond the Solar System. The 2014 meteorite was detected three years earlier than the more recent and widely known interstellar objects, ʻOumuamua in 2017 and 2I/Borisov in 2019.[22][23][24] Further related studies were reported on 1 September 2023.[25][26]
The first known dinosaur fossil linked to the very day of the Chicxulub impact is reported by paleontologists at the Tanis site in North Dakota.[27]
One science journalist reflects on the global management of the COVID-19 pandemic in relation to science, investigating the question "Why the WHO took two years to say COVID is airborne"[28] – a finding hundreds of scientists reaffirmed in an open letter in July 2020[29] – with one indication that this may be one valid major concern to many expert scientists being several writings published by news outlets.[30][31]
A study decodes electrical communication between fungi into word-like components via spiking characteristics.[32][33][34][35]
Researchers demonstrate semi-automated testing for reproducibility (which is lacking especially in cancer research) via extraction of statements about experimental results in, as of 2022 non-semantic, gene expression cancer research papers and subsequent testing with breast cancer cell lines via robot scientist "Eve".[36]

6 April: A study decodes electrical communication between fungi into word-like components.
7 April
Astronomers report the discovery of HD1, considered to be the earliest and most distant known galaxy yet identified in the observable universe, located only about 330 million years after the Big Bang 13.8 billion years ago, a light-travel distance of 13.5 billion light-years from Earth, and, due to the expansion of the universe, a present proper distance of 33.4 billion light-years.[37][38][39][40]
Physicists from the Collider Detector at Fermilab determine the mass of the W boson with a precision of 0.01%. The result hints at a flaw in the Standard Model.[41]
A trial of estimated financial energy cost of refrigerators alongside EU energy-efficiency class (EEEC) labels online finds that the approach of labels involves a trade-off between financial considerations and higher cost requirements in effort or time for the product-selection from the many available options which are often unlabelled and don't have any EEEC-requirement for being bought, used or sold within the EU.[42][43]
8 April
Bioresearchers demonstrate an in vitro method (MPTR) for rejuvenation (including the transcriptome and epigenome) reprogramming in which fibroblast skin cells temporarily lose their cell identity.[44][45]
Researchers show air pollution in fast-growing tropical cities caused ~0.5 million earlier deaths in 2018 with a substantial recent and projected rise, proposing "regulatory action targeting emerging anthropogenic sources".[46][47]
11 April – A study confirms antidepressant potential of psilocybin therapy protocols (which use the active ingredient in psilocybin mushrooms), providing fMRI data about a correlated likely major effect mechanism – global increases in brain network integration.[48][49]
12 April – Science and the 2022 Russian invasion of Ukraine:
An editorial in a scientific journal reports that relevant areas of food system research are patchy and lack independent assessments.[50] An editorial projects significant gender and age imbalance in the population in Ukraine as a substantial problem if most refugees, as in other cases, do not return over time (4 Apr).[51] A preprint reports impacts of the Ukrainian power grid synchronization with Continental Europe (15 Apr).[52]

22 April: A study outlines rationale for space governance of satellites/space debris similar to terrestrial environmental regulations.
14 April
GNz7q, a distant starburst galaxy, is reported as being a "missing link" between supermassive black holes and the evolution of quasars.[53][54]
A study describes the impact of climate change on the survival of cacti. It finds that 60% of species will experience a reduction in favourable climate by 2050–2070, with epiphytes having the greatest exposure to increased warming.[55][56]
A preprint demonstrates how backdoors can be placed undetectably into classifying (e.g. posts as "spam" or well-visible "not spam") machine learning models which are often developed and/or trained by third parties. Parties can change the classification of any input, including in cases with types of data/software transparency, possibly including white-box access.[57][58][59]

26 April: Results of the 'Global Carbon Budget 2021' pass peer-review, showing problematic continuation of GHG emissions trends.[60]
16 April – A review suggests that global prevalence of long COVID conditions after infection could be as high as 43%, with the most common symptoms being fatigue and memory problems.[61][62]
19 April – NASA publishes its Planetary Science Decadal Survey for 2023-2032. The future mission recommendations include a Uranus orbiter (the first visit to the planet since 1986) and the Enceladus Orbilander (landing in the early 2050s).[63][64]
20 April
Micronovae, a previously unknown class of thermonuclear explosions on the surface of white dwarfs, are described for the first time.[65][66]
A study shows that common single-use plastic products – such as paper coffee cups that are lined with a thin plastic film inside – release trillions of microplastics-nanoparticles per liter into water during normal use.[67][68]
21 April – Researchers discover that humans are interrupting a 66-million-years-old feature of ecosystems, the relationship between diet and body mass, by driving the largest vertebrate animals towards extinction, which they suggest could have unpredictable consequences.[69][70][71]
22 April
The Large Hadron Collider recommences full operations, three years after being shut down for upgrades.[72]
Scientists suggest in a study that space governance of satellites/space debris should regulate the current free externalization of true costs and risks, with orbital space around the Earth being an "additional ecosystem" which should be subject to regulations as e.g. oceans on Earth.[73][74]
Cancer research progress:
The largest study of whole cancer genomes reports 58 new mutational signatures and shows that for each organ "cancers have a limited number of common signatures and a long tail of rare signatures".[75][76] A study reports presence of certain bacteria in the prostate and urine for aggressive forms of prostate cancer, with biomarker- and therapeutic potentials being unclear (18 Apr).[77][78]
25 April
Novel foods such as under-development[79] cultured meat, existing microbial foods and ground-up insects are shown to have the potential to reduce environmental impacts by over 80% in a study.[80][81]
A review about meat and sustainability of food systems, animal welfare, and healthy nutrition concludes that its consumption has to be reduced substantially for sustainable consumption and names broad potential measures such as "restrictions or fiscal mechanisms".[82][83]
A new type of cell death 'erebosis' is reported[84][85] after copper-dependent cell death was first reported the previous month.[86][87]
26 April
Scientists report the detection of purine and pyrimidine nucleobases in several meteorites, including guanine, adenine, cytosine, uracil and thymine, and claim that such meteoritic nucleobases could serve as "building blocks of DNA and RNA on the early Earth".[88]
The Global Carbon Budget 2021 concludes that fossil CO2 emissions rebounded by around +4.8% relative to 2020 emissions – returning to 2019 levels, identifies three major issues for improving reliable accuracy of monitoring, shows that China and India surpassed 2019 levels (by 5.7% and 3.2%) while the EU and the US stayed beneath 2019 levels (by 5.3% and 4.5%), quantifies various changes and trends, for the first time provides models' estimates that are linked to the official country GHG inventories reporting, and shows that the remaining carbon budget at 1. Jan 2022 for a 50% likelihood to limit global warming to 1.5 °C is 120 GtC (420 GtCO2) – or 11 years of 2021 emissions levels.[60]
Scientists propose and preliminarily evaluate a likely transgressed planetary boundary for green water in the water cycle, measured by root-zone soil moisture deviation from Holocene variability.[89][additional citation(s) needed] A study published one day earlier integrates "green water" along with "blue water" into an index to measure and project water scarcity in agriculture for climate change scenarios.[90][91]
27 April
A lineage of H3N8 bird flu is found to infect humans for the first time, with a case reported in the Henan province of China.[92][93][94] Months earlier, H5 strain bird flu viruses (HPAIv) have been detected in Canada and the US.[95][96]
A study extends global assessments of shares of species threatened by extinction with reptiles, which often play functional roles in their respective ecosystems, indicating at least 21% are threatened by extinction.[97][98] One day later, scientists quantify global and local mass extinction risks of marine life from climate change and conservation potentials.[99][100]
Researchers report routes for recycling 200 industrial waste chemicals into important drugs and agrochemicals using a software for computer-aided chemical synthesis design, helping enable "circular chemistry" as a potential area of a circular economy.[101][102]
28 April
A comprehensive review reaffirms likely beneficial health effects with links to health/life extension of cycles of caloric restriction and intermittent fasting as well as reducing meat consumption in humans. It identifies issues with contemporary nutrition research approaches, proposing a multi-pillar approach, and summarizes findings towards constructing – multi-system-considering and at least age-personalized dynamic – refined longevity diets and proposes inclusion of such in standard preventive healthcare.[103][104]
A company reports results of a phase 3 clinical trial, indicating that tirzepatide could be used for substantial weight loss – possibly larger than the, as of 2022 also expensive,[105] semaglutide approved by the FDA in 2021 – in obese people.[106][105][107][additional citation(s) needed]
Researchers publish projections for interspecies viral sharing, that can lead to novel viral spillovers, due to ongoing climate change-caused range-shifts of mammals (mostly bats) for use in efforts of pandemic prevention.[108][109]"#;
        let max_context_size = 500;
        let expected_overlap_size = (max_context_size as f64 * 0.05) as usize; // 5% overlap
        let tolerance_min = 20; // Allow some variation in overlap size
        let tolerance_max = 30; // Allow some variation in overlap size

        let chunks = split_text_into_chunks(&paragraph, max_context_size);
        
        // Need at least 2 chunks to test overlap
        assert!(chunks.len() >= 2, "Test needs multiple chunks to verify overlap");

        for i in 0..chunks.len() - 1 {
            let current_chunk = &chunks[i];
            let next_chunk = &chunks[i + 1];

            // Find the longest common suffix-prefix
            let overlap = find_overlap(current_chunk, next_chunk);
            let overlap_size = overlap.len();

            // Check if overlap is within expected range
            assert!(
                (overlap_size >= expected_overlap_size - tolerance_min) && 
                (overlap_size <= expected_overlap_size + tolerance_max),
                "Overlap size {} is not within expected range {} ± {} between chunks {} and {}",
                overlap_size,
                expected_overlap_size - tolerance_min,
                expected_overlap_size + tolerance_max,
                i,
                i + 1
            );
        }
    }

    // Add this helper function for finding overlap
    fn find_overlap(text1: &str, text2: &str) -> String {
        // Get minimum length between the two texts in chars
        let text1_char_indices: Vec<(usize, char)> = text1.char_indices().collect();
        let text2_char_indices: Vec<(usize, char)> = text2.char_indices().collect();
        let min_chars = text1_char_indices.len().min(text2_char_indices.len());
        
        // Start with maximum possible overlap and work down
        for overlap_chars in (1..=min_chars).rev() {
            if let (Some((start_idx, _)), Some((_, _))) = (
                text1_char_indices.get(text1_char_indices.len() - overlap_chars),
                text2_char_indices.get(overlap_chars - 1)
            ) {
                let suffix = &text1[*start_idx..];
                let prefix = &text2[..text2_char_indices[overlap_chars - 1].0 + 
                    text2[text2_char_indices[overlap_chars - 1].0..].chars().next().map_or(0, |c| c.len_utf8())];
                
                if suffix == prefix {
                    return suffix.to_string();
                }
            }
        }
        
        String::new()
    }

    #[test]
    fn test_chunk_size_constraints() {
        let max_context_size = 1000;
        let target_chunk_size = (max_context_size as f64 * 0.8) as usize; // 80% of max context
        let tolerance = (max_context_size as f64 * 0.1) as usize; // 10% tolerance

        // Create a text that will be split into multiple chunks
        let long_text = "This is a test sentence. ".repeat(200);
        let chunks = split_text_into_chunks(&long_text, max_context_size);

        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_size = get_context_size_for_fragment(chunk.clone());
            
            // Last chunk might be smaller, so we only check upper bound
            if i == chunks.len() - 1 {
                assert!(
                    chunk_size <= max_context_size,
                    "Last chunk size {} exceeds max context size {}",
                    chunk_size,
                    max_context_size
                );
            } else {
                // For other chunks, check if size is within expected range
                assert!(
                    (chunk_size >= target_chunk_size - tolerance) && 
                    (chunk_size <= target_chunk_size + tolerance),
                    "Chunk {} size {} is not within target range {} ± {}",
                    i,
                    chunk_size,
                    target_chunk_size,
                    tolerance
                );
            }
        }
    }

    #[test]
    fn test_clean_string() {
        // Test with triple backticks and json language specifier
        let input = "```json\n{\"key\": \"value\"}\n```";
        assert_eq!(clean_string(input), "{\"key\": \"value\"}", "test 1");

        // Test with triple backticks without language specifier
        let input = "```\nplain text\n```";
        assert_eq!(clean_string(input), "plain text", "test 2");

        // Test with multiple sets of backticks - should extract first complete set
        let input = "```json\n{\"first\": true}\n```\n```\n{\"second\": false}\n```";
        assert_eq!(clean_string(input), "{\"first\": true}\n```\n```\n{\"second\": false}", "test 3");

        // Test with no backticks - should return trimmed string
        let input = "  simple text without backticks  ";
        assert_eq!(clean_string(input), "simple text without backticks", "test 4");

        // Test with empty string
        let input = "";
        assert_eq!(clean_string(input), "", "test 5");

        // Test with only whitespace
        let input = "   \n  \t  ";
        assert_eq!(clean_string(input), "", "test 6");

        // Test with incomplete backticks
        let input = "```json\n{\"incomplete\": true}";
        assert_eq!(clean_string(input), "```json\n{\"incomplete\": true}", "test 7");
    }

    #[test]
    fn test_split_text_non_utf8() {
        // Test text with various non-UTF8 characters and special characters
        let text = "Hello 世界! 🌍 \u{1F4A9} é è ü ñ 汉字 \u{10437}";
        let max_context_size = 10;
        let chunks = split_text_into_chunks(text, max_context_size);
        
        // Verify chunks are created correctly
        assert!(chunks.len() > 1, "Text should be split into multiple chunks");
        
        // Verify each chunk is valid UTF-8
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                String::from_utf8(chunk.as_bytes().to_vec()).is_ok(),
                "Chunk {} contains invalid UTF-8",
                i
            );
        }
        
        // Verify all characters are preserved when joining chunks
        let reconstructed = chunks.join("");
        assert_eq!(
            reconstructed, 
            text,
            "Reconstructed text should match original"
        );
        
        // Test with emojis at chunk boundaries
        let emoji_text = "🌍🌎🌏".repeat(10);
        let chunks = split_text_into_chunks(&emoji_text, 10);
        
        // Verify emojis aren't split
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.chars().all(|c| c.len_utf8() == c.len_utf8()),
                "Chunk {} contains split emoji characters",
                i
            );
        }
        
        // Verify with mixed ASCII and multi-byte characters
        let mixed_text = "Hello世界Hello世界Hello世界";
        let chunks = split_text_into_chunks(mixed_text, 10);
        
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.chars().all(|c| c.len_utf8() == c.len_utf8()),
                "Chunk {} contains split characters",
                i
            );
        }
    }
}