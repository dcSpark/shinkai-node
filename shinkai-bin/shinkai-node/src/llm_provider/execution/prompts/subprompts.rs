use crate::{
    llm_provider::{error::LLMProviderError, job::JobStepResult},
    managers::model_capabilities_manager::ModelCapabilitiesManager,
};
use serde::{Deserialize, Serialize};
use shinkai_vector_resources::vector_resource::{BaseVectorResource, RetrievedNode};
use std::fmt;
use tiktoken_rs::ChatCompletionRequestMessage;

use super::prompts::Prompt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SubPromptType {
    User,
    System,
    Assistant,
    ExtraContext,
}

impl fmt::Display for SubPromptType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SubPromptType::User => "user",
            SubPromptType::System => "system",
            SubPromptType::Assistant => "assistant",
            SubPromptType::ExtraContext => "user",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SubPromptAssetType {
    Image,
    Video,
    Audio,
}

pub type SubPromptAssetContent = String;
pub type SubPromptAssetDetail = String;

/// Sub-prompts are composed of a 3-element tuple of (SubPromptType, text, priority_value)
/// Priority_value is a number between 0-100, where the higher it is the less likely it will be
/// removed if LLM context window limits are reached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SubPrompt {
    Content(SubPromptType, String, u8),
    Asset(
        SubPromptType,
        SubPromptAssetType,
        SubPromptAssetContent,
        SubPromptAssetDetail,
        u8,
    ),
    EBNF(SubPromptType, String, u8, bool),
}

impl SubPrompt {
    /// Returns the length of the SubPrompt content string
    pub fn len(&self) -> usize {
        match self {
            SubPrompt::Content(_, content, _) => content.len(),
            SubPrompt::Asset(_, _, content, _, _) => content.len(),
            SubPrompt::EBNF(_, ebnf, _, _) => ebnf.len(),
        }
    }

    /// Checks if the SubPrompt content is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Generates a human/LLM-readable output string from the SubPrompt content
    pub fn generate_output_string(&self) -> String {
        match self {
            SubPrompt::Content(_, content, _) => content.clone(),
            SubPrompt::EBNF(_, ebnf, _, retry) => {
                if *retry {
                    format!("An EBNF option to respond with: {} ", ebnf)
                } else {
                    format!(
                        "Then respond using the following markdown formatting and absolutely nothing else: {} ",
                        ebnf
                    )
                }
            }
            SubPrompt::Asset(_, asset_type, _asset_content, asset_detail, _) => {
                format!("Asset Type: {:?}, Content: ..., Detail: {:?}", asset_type, asset_detail)
            }
        }
    }

    /// Extracts generic subprompt data, returning a tuple of the prompt type, content, and content type.
    pub fn extract_generic_subprompt_data(&self) -> (SubPromptType, String, &'static str) {
        match self {
            SubPrompt::Content(prompt_type, _, _) => (prompt_type.clone(), self.generate_output_string(), "text"),
            SubPrompt::EBNF(prompt_type, _, _, _) => (prompt_type.clone(), self.generate_output_string(), "text"),
            SubPrompt::Asset(prompt_type, _, asset_content, _, _) => {
                (prompt_type.clone(), asset_content.clone(), "image")
            }
        }
    }
    /// Gets the content of the SubPrompt (aka. updates it to the provided string)
    pub fn get_content(&self) -> String {
        match self {
            SubPrompt::Content(_, content, _) => content.clone(),
            SubPrompt::EBNF(_, ebnf, _, _) => ebnf.clone(),
            SubPrompt::Asset(_, _, asset_content, _, _) => asset_content.clone(),
        }
    }

    /// Sets the content of the SubPrompt (aka. updates it to the provided string)
    pub fn set_content(&mut self, new_content: String) {
        match self {
            SubPrompt::Content(_, content, _) => *content = new_content,
            SubPrompt::EBNF(_, ebnf, _, _) => *ebnf = new_content,
            SubPrompt::Asset(_, _, asset_content, _, _) => *asset_content = new_content,
        }
    }

    /// Trims the content inside of the subprompt to the specified length.
    pub fn trim_content_to_length(&mut self, max_length: usize) {
        let (_prompt_type, content, _type_) = self.extract_generic_subprompt_data();
        if content.len() > max_length {
            self.set_content(content.chars().take(max_length).collect());
        }
    }

    /// Converts a subprompt into a ChatCompletionRequestMessage
    pub fn into_chat_completion_request_message(&self) -> ChatCompletionRequestMessage {
        let (prompt_type, content, type_) = self.extract_generic_subprompt_data();
        ChatCompletionRequestMessage {
            role: prompt_type.to_string(),
            content: Some(content),
            name: if type_ == "text" { None } else { Some(type_.to_string()) },
            function_call: None,
        }
    }

    /// Counts the number of (estimated) tokens that the sub-prompt will be treated as when converted into a completion message.
    /// In other words, this is the "real" estimated token count (not just naive utf-8 character count).
    pub fn count_tokens_as_completion_message(&self) -> usize {
        let new_message = self.into_chat_completion_request_message();
        self.count_tokens_with_pregenerated_completion_message(&new_message)
    }

    /// Counts the number of (estimated) tokens that the sub-prompt will be treated as when converted into a completion message.
    /// In other words, this is the "real" estimated token count (not just naive utf-8 character count).
    /// This accepts a pregenerated completion message made from self.into_chat_completion_request_message() for greater efficiency.
    pub fn count_tokens_with_pregenerated_completion_message(
        &self,
        completion_message: &ChatCompletionRequestMessage,
    ) -> usize {
        // Only count tokens for non-image content
        let (_, _, type_) = self.extract_generic_subprompt_data();
        if type_ == "image" {
            return 0;
        }

        ModelCapabilitiesManager::num_tokens_from_llama3(&[completion_message.clone()])
    }

    /// Converts a vector resource into a series of subprompts to be used in a prompt
    /// If the VR is ordered, the output will be as well.
    pub fn convert_resource_into_subprompts(resource: &BaseVectorResource, subprompt_priority: u8) -> Vec<SubPrompt> {
        let mut temp_prompt = Prompt::new();

        let nodes = resource.as_trait_object().get_all_nodes_flattened();

        // Iterate through each node and add its text string to the prompt (which is the name of the VR)
        for node in nodes {
            if let Ok(content) = node.get_text_content() {
                temp_prompt.add_content(content.to_string(), SubPromptType::ExtraContext, subprompt_priority);
            }
            if let Ok(resource) = node.get_vector_resource_content() {
                temp_prompt.add_content(
                    resource.as_trait_object().name().to_string(),
                    SubPromptType::ExtraContext,
                    subprompt_priority,
                );
            }
        }

        temp_prompt.remove_all_subprompts()
    }
}
