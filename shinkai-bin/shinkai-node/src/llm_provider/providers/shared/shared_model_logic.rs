use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_fs::shinkai_file_manager::{FileProcessingMode, ShinkaiFileManager};
use shinkai_message_primitives::{
    schemas::{inbox_name::InboxName, llm_providers::serialized_llm_provider::LLMProviderInterface, prompts::Prompt},
    shinkai_utils::{shinkai_path::ShinkaiPath, utils::count_tokens_from_message_llama3},
};
use shinkai_sqlite::SqliteManager;

use crate::{
    llm_provider::error::LLMProviderError,
    managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResult, PromptResultEnum},
};
use shinkai_message_primitives::schemas::ws_types::{WSMessageType, WSMetadata, WSUpdateHandler};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::sync::Arc;
use tokio::sync::Mutex;

#[allow(unused)]
pub fn llama_prepare_messages(
    _model: &LLMProviderInterface,
    _model_type: String,
    prompt: Prompt,
    total_tokens: usize,
) -> Result<PromptResult, LLMProviderError> {
    let messages_string =
        prompt.generate_genericapi_messages(Some(total_tokens), &ModelCapabilitiesManager::num_tokens_from_llama3)?;

    let used_tokens = count_tokens_from_message_llama3(&messages_string);

    Ok(PromptResult {
        messages: PromptResultEnum::Text(messages_string.clone()),
        functions: None,
        remaining_output_tokens: total_tokens - used_tokens,
        tokens_used: used_tokens,
    })
}

pub fn get_image_type(base64_str: &str) -> Option<&'static str> {
    let decoded = BASE64.decode(base64_str).ok()?;
    if decoded.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpeg")
    } else if decoded.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', b'\x1A', b'\n']) {
        Some("png")
    } else if decoded.starts_with(&[b'G', b'I', b'F', b'8']) {
        Some("gif")
    } else {
        None
    }
}

pub fn get_video_type(base64_str: &str) -> Option<&'static str> {
    let decoded = BASE64.decode(base64_str).ok()?;
    if decoded.len() > 12 {
        // MP4/MOV formats usually contain the string "ftyp" starting at byte 4
        if &decoded[4..8] == b"ftyp" {
            return Some("mp4");
        }
        // WebM files start with EBML header 0x1A45DFA3
        if decoded.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
            return Some("webm");
        }
        // AVI files start with "RIFF" followed by "AVI "
        if decoded.starts_with(b"RIFF") && &decoded[8..12] == b"AVI " {
            return Some("avi");
        }
    }
    None
}

pub fn get_audio_type(base64_str: &str) -> Option<&'static str> {
    let decoded = BASE64.decode(base64_str).ok()?;
    if decoded.len() > 12 {
        // MP3 files can start with ID3 tag
        if decoded.starts_with(b"ID3") {
            return Some("mp3");
        }
        // MP3 files can also start with frame sync (0xFF followed by 0xFB, 0xFA, etc.)
        if decoded.len() > 1 && decoded[0] == 0xFF && (decoded[1] & 0xE0) == 0xE0 {
            return Some("mp3");
        }
        // WAV files start with "RIFF" followed by "WAVE"
        if decoded.starts_with(b"RIFF") && &decoded[8..12] == b"WAVE" {
            return Some("wav");
        }
        // FLAC files start with "fLaC"
        if decoded.starts_with(b"fLaC") {
            return Some("flac");
        }
        // OGG files start with "OggS"
        if decoded.starts_with(b"OggS") {
            return Some("ogg");
        }
        // M4A/AAC files have "ftyp" at byte 4 with specific brand codes
        if &decoded[4..8] == b"ftyp" && decoded.len() > 11 {
            let brand = &decoded[8..12];
            if brand == b"M4A " || brand == b"mp42" || brand == b"isom" {
                return Some("m4a");
            }
        }
        // AIFF files start with "FORM" followed by "AIFF"
        if decoded.starts_with(b"FORM") && &decoded[8..12] == b"AIFF" {
            return Some("aiff");
        }
    }
    None
}

/// Save an image file from base64 data to the job's file storage
pub async fn save_image_file(
    mime_type: &str,
    base64_data: &str,
    inbox_name: &Option<InboxName>,
    session_id: &str,
    db: &SqliteManager,
) -> Result<ShinkaiPath, LLMProviderError> {
    // Decode base64 image data
    let image_data = BASE64
        .decode(base64_data)
        .map_err(|e| LLMProviderError::NetworkError(format!("Failed to decode base64 image data: {}", e)))?;

    // Create a unique filename for the image
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let file_extension = match mime_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "png", // Default to png
    };

    let filename = format!("generated_image_{}_{}.{}", session_id, timestamp, file_extension);

    // Extract job ID from inbox name using the proper method
    let job_id = if let Some(inbox) = inbox_name {
        match inbox.get_job_id() {
            Some(job_id) => job_id,
            None => {
                return Err(LLMProviderError::NetworkError(
                    "Inbox is not a job inbox - cannot save images".to_string(),
                ));
            }
        }
    } else {
        return Err(LLMProviderError::NetworkError(
            "Inbox name is required for saving images".to_string(),
        ));
    };

    // Create a default embedding generator
    let embedding_generator = RemoteEmbeddingGenerator::new_default();

    // Save the image file with the job ID - this will use the proper job-based file organization
    let shinkai_path = ShinkaiFileManager::save_and_process_file_with_jobid(
        &job_id,
        filename.clone(),
        image_data,
        db,
        FileProcessingMode::NoParsing, // Images don't contain parseable text, but file is still saved to job context
        &embedding_generator,
    )
    .await
    .map_err(|e| LLMProviderError::NetworkError(format!("Failed to save image file: {}", e)))?;

    Ok(shinkai_path)
}

/// Send a WebSocket update message through the provided manager
pub async fn send_ws_update(
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: Option<InboxName>,
    session_id: &str,
    content: String,
    is_reasoning: bool,
    is_done: bool,
    done_reason: Option<String>,
) -> Result<(), LLMProviderError> {
    if let Some(ref manager) = ws_manager_trait {
        if let Some(inbox_name) = inbox_name {
            let m = manager.lock().await;
            let inbox_name_string = inbox_name.to_string();

            let metadata = WSMetadata {
                id: Some(session_id.to_string()),
                is_reasoning,
                is_done,
                done_reason,
                total_duration: None,
                eval_count: None,
            };

            let ws_message_type = WSMessageType::Metadata(metadata);

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Websocket content: {}", content).as_str(),
            );

            let _ = m
                .queue_message(WSTopic::Inbox, inbox_name_string, content, ws_message_type, true)
                .await;
        }
    }
    Ok(())
}

/// Send a tool WebSocket update message through the provided manager
pub async fn send_tool_ws_update(
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: Option<InboxName>,
    function_call: &crate::llm_provider::execution::chains::inference_chain_trait::FunctionCall,
) -> Result<(), LLMProviderError> {
    send_tool_ws_update_with_status(ws_manager_trait, inbox_name, function_call, None, None).await
}

/// Send a tool WebSocket update message through the provided manager with custom status and result
pub async fn send_tool_ws_update_with_status(
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: Option<InboxName>,
    function_call: &crate::llm_provider::execution::chains::inference_chain_trait::FunctionCall,
    result: Option<serde_json::Value>,
    status_type: Option<shinkai_message_primitives::schemas::ws_types::ToolStatusType>,
) -> Result<(), LLMProviderError> {
    use shinkai_message_primitives::schemas::ws_types::{
        ToolMetadata, ToolStatus, ToolStatusType, WSMessageType, WidgetMetadata,
    };

    if let Some(ref manager) = ws_manager_trait {
        if let Some(inbox_name) = inbox_name {
            let m = manager.lock().await;
            let inbox_name_string = inbox_name.to_string();

            let function_call_json = serde_json::to_value(function_call).unwrap_or_else(|_| serde_json::json!({}));

            let tool_metadata = ToolMetadata {
                tool_name: function_call.name.clone(),
                tool_router_key: function_call.tool_router_key.clone(),
                args: function_call_json.as_object().cloned().unwrap_or_default(),
                result,
                status: ToolStatus {
                    type_: status_type.unwrap_or(ToolStatusType::Running),
                    reason: None,
                },
                index: function_call.index,
            };

            let ws_message_type = WSMessageType::Widget(WidgetMetadata::ToolRequest(tool_metadata));

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!(
                    "Websocket content (function_call): {}",
                    serde_json::to_string(function_call).unwrap_or_else(|_| "{}".to_string())
                )
                .as_str(),
            );

            let _ = m
                .queue_message(
                    WSTopic::Inbox,
                    inbox_name_string,
                    serde_json::to_string(function_call).unwrap_or_else(|_| "{}".to_string()),
                    ws_message_type,
                    true,
                )
                .await;
        }
    }
    Ok(())
}

pub fn sanitize_tool_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();

    let mut result = if sanitized.is_empty() {
        "tool".to_string()
    } else {
        sanitized.chars().take(64).collect()
    };

    // Ensure the name starts with a letter or underscore
    if let Some(first_char) = result.chars().next() {
        if !first_char.is_alphabetic() && first_char != '_' {
            result = format!("t_{}", result);
        }
    }

    // Ensure length is still within 64 characters after potential prefix
    result.chars().take(64).collect()
}
