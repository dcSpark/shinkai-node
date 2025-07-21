use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use shinkai_message_primitives::{schemas::{
    llm_providers::serialized_llm_provider::LLMProviderInterface, prompts::Prompt,
}, shinkai_utils::utils::count_tokens_from_message_llama3};

use crate::{
    llm_provider::error::LLMProviderError,
    managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResult, PromptResultEnum},
};

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
