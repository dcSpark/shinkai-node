use crate::{
    llm_provider::{error::LLMProviderError, job::JobStepResult},
    managers::model_capabilities_manager::ModelCapabilitiesManager,
};
use serde::{Deserialize, Serialize};
use shinkai_vector_resources::vector_resource::{BaseVectorResource, RetrievedNode};
use std::{collections::HashMap, fmt};
use tiktoken_rs::ChatCompletionRequestMessage;

pub struct JobPromptGenerator {}

impl JobPromptGenerator {
    /// Parses an execution context hashmap to string to be added into a content subprompt
    pub fn parse_context_to_string(context: HashMap<String, String>) -> String {
        context
            .into_iter()
            .map(|(key, value)| format!("{}: {}", key, value))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

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

    /// Adds an ebnf sub-prompt.
    /// Of note, priority value must be between 0-100, where higher is greater priority
    pub fn add_ebnf(&mut self, ebnf: String, prompt_type: SubPromptType, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100);
        let sub_prompt = SubPrompt::EBNF(prompt_type, ebnf, capped_priority_value as u8, false);
        self.add_sub_prompt(sub_prompt);
    }

    /// Adds an ebnf sub-prompt that is meant for retry prompts.
    /// Of note, priority value must be between 0-100, where higher is greater priority
    pub fn add_retry_ebnf(&mut self, ebnf: String, prompt_type: SubPromptType, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100);
        let sub_prompt = SubPrompt::EBNF(prompt_type, ebnf, capped_priority_value as u8, true);
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

    /// Validates that there is at least one EBNF sub-prompt to ensure
    /// the LLM knows what to output.
    pub fn check_ebnf_included(&self) -> Result<(), LLMProviderError> {
        if !self
            .sub_prompts
            .iter()
            .any(|prompt| matches!(prompt, SubPrompt::EBNF(_, _, _, _)))
        {
            return Err(LLMProviderError::UserPromptMissingEBNFDefinition);
        }
        Ok(())
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

    /// Generates a tuple of a list of ChatCompletionRequestMessages and their token length,
    /// ready to be used with OpenAI inferencing.
    // fn generate_chat_completion_messages(&self) -> (Vec<ChatCompletionRequestMessage>, usize) {
    //     let mut tiktoken_messages: Vec<ChatCompletionRequestMessage> = Vec::new();
    //     let mut current_length: usize = 0;

    //     eprintln!("sub_prompts: {:?}", self.sub_prompts);

    //     // Process all sub-prompts in their original order
    //     for sub_prompt in &self.sub_prompts {
    //         let new_message = sub_prompt.into_chat_completion_request_message();
    //         // Nico: fix
    //         current_length += sub_prompt.count_tokens_with_pregenerated_completion_message(&new_message);
    //         tiktoken_messages.push(new_message);
    //     }

    //     (tiktoken_messages, current_length)
    // }
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

//
// Core Job Step Flow
//
// Note this will all happen within a single Job step. We will probably end up summarizing the context/results from previous steps into the step history to be included as the base initial context for new steps.
//
// 0. User submits an initial message/request to their AI Agent.
// 1. An initial bootstrap plan is created based on the initial request from the user.
//
// 2. We enter into "analysis phase".
// 3a. Iterating starting from the first point in the plan, we ask the LLM true/false if it can provide an answer given it's personal knowledge + current context.
// 3b. If it can then we mark this analysis step as "prepared" and go back to 3a for the next bootstrap plan task.
// 3c. If not we tell the LLM to search for tools that would work for this task.
// 4a. We return a list of tools to it, and ask it to either select one, or return an error message.
// 4b. If it returns an error message, it means the plan can not be completed/Agent has failed, and we exit/send message back to user with the error message (15).
// 4c. If it chooses one, we fetch the tool info including the input EBNF.
// 5a. We now show the input EBNF to the LLM, and ask it whether or not it has all the needed knowledge + potential data in the current context to be able to use the tool. (In either case  after the LLM chooses)
// 5b. The LLM says it has all the needed info, then we add the tool's name/input EBNF to the current context, and either go back to 3a for the next bootstrap plan task if the task is now finished/prepared, or go to 6 if this tool was searched for to find an input for another tool.
// 5c. The LLM doesn't have all the info it needs, so it performs another tool search and we go back to 4a.
// 6. After resolving 4-5 for the new tool search, the new tool's input EBNF has been added into the context window, which will allow us to go back to 5a for the original tool, which enables the LLM to now state it has all the info it needs (marking the analysis step as prepared), thus going back to 3a for the next top level task.
// 7. After iterating through all the bootstrap plan tasks and analyzing them, we have created an "execution plan" that specifies all tool calls which will need to be made.
//
// 8. We now move to the "execution phase".
// 9. Using the execution plan, we move forward alternating between inferencing the LLM and executing a tool, as dictated by the plan.
// 10. To start we inference the LLM with the first step in the plan + the input EBNF of the first tool, and tell the LLM to fill out the input EBNF with real data.
// 11. The input JSON is taken and the tool is called/executed, with the results being added into the context.
// 12. With the tool executed, we now inference the LLM with just the context + the input EBNF of the next tool that it needs to fill out (we can skip user's request text).
// 13. We iterate through the entire execution plan (looping back/forth between 11/12) and arrive at the end with a context filled with all relevant data needed to answer the user's initial request.
// 14. We inference the LLM one last time, providing it just the context + list of executed tools, and telling it to respond to the user's request by using/summarizing the results.
// 15. We add a Shinkai message into the job's inbox with the LLM's response, allowing the user to see the result.
//
//
//
//

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
