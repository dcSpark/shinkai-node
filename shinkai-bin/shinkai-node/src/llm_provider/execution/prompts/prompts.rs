use crate::{
    llm_provider::{error::LLMProviderError, job::JobStepResult},
    managers::model_capabilities_manager::ModelCapabilitiesManager,
};
use serde::{Deserialize, Serialize};
use shinkai_vector_resources::vector_resource::{BaseVectorResource, RetrievedNode};
use std::fmt;
use tiktoken_rs::ChatCompletionRequestMessage;

use super::subprompts::{SubPrompt, SubPromptAssetContent, SubPromptAssetDetail, SubPromptAssetType, SubPromptType};

pub struct JobPromptGenerator {}

/// Struct that represents a prompt to be used for inferencing an LLM
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    /// Sub-prompts that make up this prompt
    pub sub_prompts: Vec<SubPrompt>,
    /// The lowest priority value held in sub_prompts. TODO: Make this a hashmap to make it more efficient for updating priorities.
    pub lowest_priority: u8,
    /// The highest priority value held in sub_prompts. TODO: Make this a hashmap to make it more efficient for updating priorities.
    pub highest_priority: u8,
    /// Does it accept a # Answer compliant response?
    pub accept_non_ebnf: Option<bool>,
}

impl Default for Prompt {
    fn default() -> Self {
        Self::new()
    }
}

impl Prompt {
    pub fn new() -> Self {
        Self {
            sub_prompts: Vec::new(),
            lowest_priority: 100,
            highest_priority: 0,
            accept_non_ebnf: None,
        }
    }

    pub fn to_json(&self) -> Result<String, LLMProviderError> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(json: &str) -> Result<Self, LLMProviderError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Adds a sub-prompt that holds any String content.
    /// Of note, priority value must be between 0-100, where higher is greater priority
    pub fn add_content(&mut self, content: String, prompt_type: SubPromptType, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100);
        let sub_prompt = SubPrompt::Content(prompt_type, content, capped_priority_value as u8);
        self.add_sub_prompt(sub_prompt);
    }

    /// Adds RetrievedNode content into the prompt if it is a Text-holding node. Otherwise skips.
    pub fn add_ret_node_content(
        &mut self,
        retrieved_node: RetrievedNode,
        prompt_type: SubPromptType,
        priority_value: u8,
    ) {
        if let Some(content) = retrieved_node.format_for_prompt(3500) {
            self.add_content(content, prompt_type, priority_value);
        }
    }

    /// Adds a sub-prompt that holds an Asset.
    /// Of note, priority value must be between 0-100, where higher is greater priority
    pub fn add_asset(
        &mut self,
        asset_type: SubPromptAssetType,
        asset_content: SubPromptAssetContent,
        asset_detail: SubPromptAssetDetail,
        prompt_type: SubPromptType,
        priority_value: u8,
    ) {
        let capped_priority_value = std::cmp::min(priority_value, 100);
        let sub_prompt = SubPrompt::Asset(
            prompt_type,
            asset_type,
            asset_content,
            asset_detail,
            capped_priority_value as u8,
        );
        self.add_sub_prompt(sub_prompt);
    }

    /// Updates the lowest and highest priority values of self using the
    /// existing priority values of the sub_prompts.
    fn update_sub_prompts_priorities(&mut self) {
        // Set to the defaults, which get updated if there are sub_prompts
        self.lowest_priority = 100;
        self.highest_priority = 0;

        for sub_prompt in self.sub_prompts.iter() {
            match &sub_prompt {
                SubPrompt::Content(_, _, priority) | SubPrompt::EBNF(_, _, priority, _) => {
                    self.lowest_priority = self.lowest_priority.min(*priority);
                    self.highest_priority = self.highest_priority.max(*priority);
                }
                SubPrompt::Asset(_, _, _, _, priority) => {
                    self.lowest_priority = self.lowest_priority.min(*priority);
                    self.highest_priority = self.highest_priority.max(*priority);
                }
            }
        }
    }

    /// Adds a single sub-prompt.
    /// Updates the lowest and highest priority values of self
    pub fn add_sub_prompt(&mut self, sub_prompt: SubPrompt) {
        self.add_sub_prompts(vec![sub_prompt]);
    }

    /// Adds multiple pre-prepared sub-prompts.
    /// Updates the lowest and highest priority values of self
    pub fn add_sub_prompts(&mut self, mut sub_prompts: Vec<SubPrompt>) {
        self.sub_prompts.append(&mut sub_prompts);
        self.update_sub_prompts_priorities();
    }

    /// Remove sub prompt at index
    /// Updates the lowest and highest priority values of self
    pub fn remove_sub_prompt(&mut self, index: usize) -> SubPrompt {
        let element = self.sub_prompts.remove(index);
        self.update_sub_prompts_priorities();
        element
    }

    /// Remove sub prompt at index safely, or returns None.
    /// Updates the lowest and highest priority values of self
    pub fn remove_sub_prompt_safe(&mut self, index: usize) -> Option<SubPrompt> {
        if index < self.sub_prompts.len() {
            let element = self.remove_sub_prompt(index);
            Some(element)
        } else {
            None
        }
    }

    /// Adds multiple pre-prepared sub-prompts with a new priority value.
    /// The new priority value will be applied to all input sub-prompts.
    pub fn add_sub_prompts_with_new_priority(&mut self, sub_prompts: Vec<SubPrompt>, new_priority: u8) {
        let capped_priority_value = std::cmp::min(new_priority, 100) as u8;
        let mut updated_sub_prompts = Vec::new();
        for mut sub_prompt in sub_prompts {
            match &mut sub_prompt {
                SubPrompt::Content(_, _, priority) | SubPrompt::EBNF(_, _, priority, _) => {
                    *priority = capped_priority_value
                }
                SubPrompt::Asset(_, _, _, _, priority) => *priority = capped_priority_value,
            }
            updated_sub_prompts.push(sub_prompt);
        }
        self.add_sub_prompts(updated_sub_prompts);
    }

    /// Adds previous results from step history into the Prompt, up to max_tokens
    /// Of note, priority value must be between 0-100.
    pub fn add_step_history(&mut self, history: Vec<JobStepResult>, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100) as u8;
        let sub_prompts_list: Vec<SubPrompt> = history
            .iter()
            .filter_map(|step| step.get_result_prompt())
            .flat_map(|prompt| prompt.sub_prompts.clone())
            .collect();
        self.add_sub_prompts_with_new_priority(sub_prompts_list, capped_priority_value);
    }

    /// Removes the first sub-prompt from the end of the sub_prompts list that has the lowest priority value.
    /// Used primarily for cutting down prompt when it is too large to fit in context window.
    pub fn remove_lowest_priority_sub_prompt(&mut self) -> Option<SubPrompt> {
        let lowest_priority = self.lowest_priority;
        if let Some(position) = self.sub_prompts.iter().rposition(|sub_prompt| match sub_prompt {
            SubPrompt::Content(_, _, priority) | SubPrompt::EBNF(_, _, priority, _) => *priority == lowest_priority,
            SubPrompt::Asset(_, _, _, _, priority) => *priority == lowest_priority,
        }) {
            return Some(self.remove_sub_prompt(position));
        }
        None
    }

    /// Removes lowest priority sub-prompts until the total token count is under the specified cap.
    /// Returns the sub-prompts that were removed, in the same order that they were in.
    pub fn remove_subprompts_until_under_max(&mut self, max_prompt_tokens: usize) -> Vec<SubPrompt> {
        let mut removed_subprompts = vec![];

        let mut current_token_count = self.generate_chat_completion_messages().1;
        while current_token_count + 200 > max_prompt_tokens {
            match self.remove_lowest_priority_sub_prompt() {
                Some(removed_sub_prompt) => {
                    current_token_count -= removed_sub_prompt.count_tokens_as_completion_message();
                    removed_subprompts.push(removed_sub_prompt);
                }
                None => break, // No more sub-prompts to remove, exit the loop
            }
        }

        removed_subprompts.reverse();
        removed_subprompts
    }

    /// Removes all sub-prompts from the prompt.
    pub fn remove_all_subprompts(&mut self) -> Vec<SubPrompt> {
        let removed_subprompts = self.sub_prompts.drain(..).collect();
        self.update_sub_prompts_priorities();
        removed_subprompts
    }

    /// Processes all sub-prompts into a single output String.
    pub fn generate_single_output_string(&self) -> Result<String, LLMProviderError> {
        let content = self
            .sub_prompts
            .iter()
            .map(|sub_prompt| sub_prompt.generate_output_string())
            .collect::<Vec<String>>()
            .join("\n")
            + "\n";
        Ok(content)
    }

    fn generate_chat_completion_messages(&self) -> (Vec<ChatCompletionRequestMessage>, usize) {
        let mut tiktoken_messages: Vec<ChatCompletionRequestMessage> = Vec::new();
        let mut current_length: usize = 0;

        // Accumulator for ExtraContext content
        let mut extra_context_content = String::new();
        let mut processing_extra_context = false;

        for sub_prompt in &self.sub_prompts {
            match sub_prompt {
                // Accumulate ExtraContext content
                SubPrompt::Content(SubPromptType::ExtraContext, content, _) => {
                    extra_context_content.push_str(content);
                    extra_context_content.push('\n');
                    processing_extra_context = true;
                }
                _ => {
                    // If we were processing ExtraContext, add it as a single System message
                    if processing_extra_context {
                        let extra_context_message = ChatCompletionRequestMessage {
                            role: SubPromptType::System.to_string(),
                            content: Some(extra_context_content.trim().to_string()),
                            name: None,
                            function_call: None,
                        };
                        current_length +=
                            ModelCapabilitiesManager::num_tokens_from_llama3(&[extra_context_message.clone()]);
                        tiktoken_messages.push(extra_context_message);

                        // Reset the accumulator
                        extra_context_content.clear();
                        processing_extra_context = false;
                    }

                    // Process the current sub-prompt
                    let new_message = sub_prompt.into_chat_completion_request_message();
                    current_length += sub_prompt.count_tokens_with_pregenerated_completion_message(&new_message);
                    tiktoken_messages.push(new_message);
                }
            }
        }

        // If there are any remaining ExtraContext sub-prompts, add them as a single message
        if processing_extra_context && !extra_context_content.is_empty() {
            let extra_context_message = ChatCompletionRequestMessage {
                role: SubPromptType::System.to_string(),
                content: Some(extra_context_content.trim().to_string()),
                name: None,
                function_call: None,
            };
            current_length += ModelCapabilitiesManager::num_tokens_from_llama3(&[extra_context_message.clone()]);
            tiktoken_messages.push(extra_context_message);
        }

        (tiktoken_messages, current_length)
    }

    /// Processes all sub-prompts into a single output String in OpenAI's message format.
    pub fn generate_openai_messages(
        &self,
        max_prompt_tokens: Option<usize>,
    ) -> Result<Vec<ChatCompletionRequestMessage>, LLMProviderError> {
        // We take about half of a default total 4097 if none is provided as a backup (should never happen)
        let limit = max_prompt_tokens.unwrap_or(2700_usize);

        // Remove sub-prompts until the total token count is under the specified limit
        let mut prompt_copy = self.clone();
        prompt_copy.remove_subprompts_until_under_max(limit);

        // Generate the output chat completion request messages
        let (output_messages, _) = prompt_copy.generate_chat_completion_messages();

        Ok(output_messages)
    }

    // Generates generic api messages as a single string.
    // TODO: needs to be updated
    pub fn generate_genericapi_messages(&self, max_input_tokens: Option<usize>) -> Result<String, LLMProviderError> {
        // let limit = max_input_tokens.unwrap_or(4000 as usize);
        let limit = max_input_tokens.unwrap_or(4000_usize);
        let mut prompt_copy = self.clone();
        prompt_copy.remove_subprompts_until_under_max(limit);

        let mut messages: Vec<String> = Vec::new();
        // Process all sub-prompts in their original order
        for sub_prompt in prompt_copy.sub_prompts.iter() {
            match sub_prompt {
                SubPrompt::Asset(_, _, _, _, _) => {
                    // Ignore Asset
                }
                SubPrompt::Content(prompt_type, content, _priority_value) => {
                    let mut new_message = "".to_string();
                    if prompt_type == &SubPromptType::System || prompt_type == &SubPromptType::Assistant {
                        new_message = format!("Sys: {}\n", content.clone());
                    } else if prompt_type == &SubPromptType::User {
                        new_message = format!("User: {}\n", content.clone());
                    } else if prompt_type == &SubPromptType::Assistant {
                        new_message = format!("A: {}\n", content.clone());
                    }
                    messages.push(new_message);
                }
                SubPrompt::EBNF(_, content, _, _) => {
                    let new_message = format!("{}\n", content.clone());
                    messages.push(new_message);
                }
            }
        }
        let output = messages.join(" ");
        // eprintln!("generate_genericapi_messages output: {:?}", output);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_chat_completion_messages() {
        let sub_prompts = vec![
            SubPrompt::Content(SubPromptType::System, "You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user information using paragraphs, blocks, and bulletpoint lists. Use the content to directly answer the user's question. If the user talks about `it` or `this`, they are referencing the previous message.\n Respond using the following markdown schema and nothing else:\n # Answer \nhere goes the answer\n".to_string(), 98),
            SubPrompt::Content(SubPromptType::User, "summarize this".to_string(), 97),
            SubPrompt::Content(SubPromptType::Assistant, "## What are the benefits of using Vector Resources ...\n\n".to_string(), 97),
            SubPrompt::Content(SubPromptType::ExtraContext, "Here is a list of relevant new content provided for you to potentially use while answering:".to_string(), 97),
            SubPrompt::Content(SubPromptType::ExtraContext, "- FAQ Shinkai Overview What’s Shinkai? (Summary)  (Source: Shinkai - Ask Me Anything.docx, Section: ) 2024-05-05T00:33:00".to_string(), 97),
            SubPrompt::Content(SubPromptType::ExtraContext, "- Shinkai is a comprehensive super app designed to enhance how users interact with AI. It allows users to run AI locally, facilitating direct conversations with documents and managing files converted into AI embeddings for advanced semantic searches across user data. This local execution ensures privacy and efficiency, putting control directly in the user's hands.  (Source: Shinkai - Ask Me Anything.docx, Section: 2) 2024-05-05T00:33:00".to_string(), 97),
            SubPrompt::Content(SubPromptType::User, "tell me more about Shinkai. Answer the question using this markdown and the extra context provided: \n # Answer \n here goes the answer\n".to_string(), 100),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        let (messages, _token_length) = prompt.generate_chat_completion_messages();

        // Expected messages
        let expected_messages = vec![
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user information using paragraphs, blocks, and bulletpoint lists. Use the content to directly answer the user's question. If the user talks about `it` or `this`, they are referencing the previous message.\n Respond using the following markdown schema and nothing else:\n # Answer \nhere goes the answer\n".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "user".to_string(),
                content: Some("summarize this".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "assistant".to_string(),
                content: Some("## What are the benefits of using Vector Resources ...\n\n".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("Here is a list of relevant new content provided for you to potentially use while answering:\n- FAQ Shinkai Overview What’s Shinkai? (Summary)  (Source: Shinkai - Ask Me Anything.docx, Section: ) 2024-05-05T00:33:00\n- Shinkai is a comprehensive super app designed to enhance how users interact with AI. It allows users to run AI locally, facilitating direct conversations with documents and managing files converted into AI embeddings for advanced semantic searches across user data. This local execution ensures privacy and efficiency, putting control directly in the user's hands.  (Source: Shinkai - Ask Me Anything.docx, Section: 2) 2024-05-05T00:33:00".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "user".to_string(),
                content: Some("tell me more about Shinkai. Answer the question using this markdown and the extra context provided: \n # Answer \n here goes the answer\n".to_string()),
                name: None,
                function_call: None,
            },
        ];

        // Check if the generated messages match the expected messages
        assert_eq!(messages, expected_messages);
    }
}
