use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_vector_resources::vector_resource::RetrievedNode;

use super::{
    job::JobStepResult,
    llm_message::{DetailedFunctionCall, LlmMessage},
    subprompts::{SubPrompt, SubPromptAssetContent, SubPromptAssetDetail, SubPromptAssetType, SubPromptType},
};

#[derive(Debug)]
pub enum PromptError {
    SerializationError(serde_json::Error),
    DeserializationError(serde_json::Error),
    // Add other error variants as needed
}

impl fmt::Display for PromptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PromptError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            PromptError::DeserializationError(e) => write!(f, "Deserialization error: {}", e),
            // Handle other error variants
        }
    }
}

impl std::error::Error for PromptError {}

impl From<serde_json::Error> for PromptError {
    fn from(error: serde_json::Error) -> Self {
        if error.is_data() {
            PromptError::DeserializationError(error)
        } else {
            PromptError::SerializationError(error)
        }
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
        }
    }

    pub fn to_json(&self) -> Result<String, PromptError> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(json: &str) -> Result<Self, PromptError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Adds a sub-prompt that holds any String content.
    /// Of note, priority value must be between 0-100, where higher is greater priority
    pub fn add_content(&mut self, content: String, prompt_type: SubPromptType, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100);
        let sub_prompt = SubPrompt::Content(prompt_type, content, capped_priority_value as u8);
        self.add_sub_prompt(sub_prompt);
    }

    /// Adds a sub-prompt that holds any Omni (String + Assets) content.
    /// Of note, priority value must be between 0-100, where higher is greater priority
    pub fn add_omni(
        &mut self,
        content: String,
        files: HashMap<String, String>,
        prompt_type: SubPromptType,
        priority_value: u8,
    ) {
        let capped_priority_value = std::cmp::min(priority_value, 100);
        let assets: Vec<(SubPromptAssetType, SubPromptAssetContent, SubPromptAssetDetail)> = files
            .into_iter()
            // TODO: later on we will want to add more asset types. Do we really need the SubPromptAssetType?
            .map(|(file_name, file_content)| (SubPromptAssetType::Image, file_content, file_name))
            .collect();
        let sub_prompt = SubPrompt::Omni(prompt_type, content, assets, capped_priority_value as u8);
        self.add_sub_prompt(sub_prompt);
    }

    /// Adds a sub-prompt that holds a Tool.
    /// Of note, priority value must be between 0-100, where higher is greater priority
    pub fn add_tool(&mut self, tool_content: serde_json::Value, prompt_type: SubPromptType, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100);
        let sub_prompt = SubPrompt::ToolAvailable(prompt_type, tool_content, capped_priority_value as u8);
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
            if !content.trim().is_empty() {
                self.add_content(content, prompt_type, priority_value);
            }
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

    /// Adds a sub-prompt that holds a function call by the assistant.
    /// Of note, priority value must be between 0-100, where higher is greater priority
    pub fn add_function_call(&mut self, function_call: Value, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100);
        let sub_prompt = SubPrompt::FunctionCall(SubPromptType::Assistant, function_call, capped_priority_value as u8);

        self.add_sub_prompt(sub_prompt);
    }

    /// Adds a sub-prompt that holds a function call response.
    /// Of note, priority value must be between 0-100, where higher is greater priority
    pub fn add_function_call_response(&mut self, function_call_response: Value, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100);
        let sub_prompt = SubPrompt::FunctionCallResponse(
            SubPromptType::Function,
            function_call_response,
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
                SubPrompt::Content(_, _, priority) | SubPrompt::ToolAvailable(_, _, priority) => {
                    self.lowest_priority = self.lowest_priority.min(*priority);
                    self.highest_priority = self.highest_priority.max(*priority);
                }
                SubPrompt::Asset(_, _, _, _, priority) => {
                    self.lowest_priority = self.lowest_priority.min(*priority);
                    self.highest_priority = self.highest_priority.max(*priority);
                }
                SubPrompt::FunctionCall(_, _, priority) => {
                    self.lowest_priority = self.lowest_priority.min(*priority);
                    self.highest_priority = self.highest_priority.max(*priority);
                }
                SubPrompt::FunctionCallResponse(_, _, priority) => {
                    self.lowest_priority = self.lowest_priority.min(*priority);
                    self.highest_priority = self.highest_priority.max(*priority);
                }
                SubPrompt::Omni(_, _, _, priority) => {
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
                SubPrompt::Content(_, _, priority) | SubPrompt::ToolAvailable(_, _, priority) => {
                    *priority = capped_priority_value
                }
                SubPrompt::Asset(_, _, _, _, priority) => *priority = capped_priority_value,
                SubPrompt::FunctionCall(_, _, priority) => *priority = capped_priority_value,
                SubPrompt::FunctionCallResponse(_, _, priority) => *priority = capped_priority_value,
                SubPrompt::Omni(_, _, _, priority) => *priority = capped_priority_value,
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
            SubPrompt::Content(_, _, priority) | SubPrompt::ToolAvailable(_, _, priority) => {
                *priority == lowest_priority
            }
            SubPrompt::Asset(_, _, _, _, priority) => *priority == lowest_priority,
            SubPrompt::FunctionCall(_, _, priority) => *priority == lowest_priority,
            SubPrompt::FunctionCallResponse(_, _, priority) => *priority == lowest_priority,
            SubPrompt::Omni(_, _, _, priority) => *priority == lowest_priority,
        }) {
            return Some(self.remove_sub_prompt(position));
        }
        None
    }

    /// Removes lowest priority sub-prompts until the total token count is under the specified cap.
    /// Returns the sub-prompts that were removed, in the same order that they were in.
    pub fn remove_subprompts_until_under_max<F>(
        &mut self,
        max_prompt_tokens: usize,
        token_counter: &F,
    ) -> Vec<SubPrompt>
    where
        F: Fn(&[LlmMessage]) -> usize,
    {
        let mut removed_subprompts = vec![];

        let mut current_token_count = self.generate_chat_completion_messages(None, token_counter).1;
        while current_token_count + 200 > max_prompt_tokens {
            match self.remove_lowest_priority_sub_prompt() {
                Some(removed_sub_prompt) => {
                    let removed_tokens = removed_sub_prompt.count_tokens_as_completion_message(token_counter);
                    if current_token_count >= removed_tokens {
                        current_token_count -= removed_tokens;
                    } else {
                        current_token_count = 0;
                    }
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
    pub fn generate_single_output_string(&self) -> Result<String, PromptError> {
        let content = self
            .sub_prompts
            .iter()
            .map(|sub_prompt| sub_prompt.generate_output_string())
            .collect::<Vec<String>>()
            .join("\n")
            + "\n";
        Ok(content)
    }

    pub fn generate_chat_completion_messages<F>(
        &self,
        tool_response_field_name: Option<String>,
        token_counter: &F,
    ) -> (Vec<LlmMessage>, usize)
    where
        F: Fn(&[LlmMessage]) -> usize,
    {
        let mut tiktoken_messages: Vec<LlmMessage> = Vec::new();
        let mut current_length: usize = 0;

        // Accumulator for ExtraContext content
        let mut extra_context_content = String::new();
        let mut last_user_message: Option<LlmMessage> = None;
        let mut function_calls: Vec<LlmMessage> = Vec::new();
        let mut function_call_responses: Vec<LlmMessage> = Vec::new();

        for sub_prompt in &self.sub_prompts {
            match sub_prompt {
                // Accumulate ExtraContext content
                SubPrompt::Content(SubPromptType::ExtraContext, content, _) => {
                    extra_context_content.push_str(content);
                    extra_context_content.push('\n');
                }
                SubPrompt::ToolAvailable(_, content, _) => {
                    let tool_message = LlmMessage::import_functions_from_value(content.clone()).unwrap();
                    current_length +=
                        sub_prompt.count_tokens_with_pregenerated_completion_message(&tool_message, token_counter);
                    tiktoken_messages.push(tool_message);
                }
                SubPrompt::FunctionCall(_, content, _) => {
                    let mut new_message = LlmMessage {
                        role: Some("assistant".to_string()),
                        content: None,
                        name: None,
                        function_call: None,
                        functions: None,
                        images: None,
                    };

                    if let Some(name) = content.get("name").and_then(|n| n.as_str()) {
                        let arguments = content
                            .get("arguments")
                            .map_or_else(|| "".to_string(), |args| args.to_string());
                        new_message.function_call = Some(DetailedFunctionCall {
                            name: name.to_string(),
                            arguments,
                        });
                    }
                    current_length +=
                        sub_prompt.count_tokens_with_pregenerated_completion_message(&new_message, token_counter);
                    function_calls.push(new_message);
                }
                SubPrompt::FunctionCallResponse(_, content, _) => {
                    let mut new_message = LlmMessage {
                        // OpenAI works using "function" while ollama uses "tool"
                        role: tool_response_field_name.clone().or(Some("function".to_string())),
                        content: None,
                        name: None,
                        function_call: None,
                        functions: None,
                        images: None,
                    };

                    if let Some(function_call) = content.get("function_call") {
                        if let Some(name) = function_call.get("name").and_then(|n| n.as_str()) {
                            new_message.name = Some(name.to_string());
                        }
                    }
                    new_message.content = content.get("response").and_then(|r| r.as_str()).map(|r| r.to_string());

                    current_length +=
                        sub_prompt.count_tokens_with_pregenerated_completion_message(&new_message, token_counter);
                    function_call_responses.push(new_message);
                }
                SubPrompt::Content(SubPromptType::UserLastMessage, content, _) => {
                    last_user_message = Some(LlmMessage {
                        role: Some(SubPromptType::User.to_string()),
                        content: Some(content.clone()),
                        name: None,
                        function_call: None,
                        functions: None,
                        images: None,
                    });
                }
                SubPrompt::Omni(prompt_type, _, _, _) => {
                    // Process the current sub-prompt
                    let new_message = sub_prompt.into_chat_completion_request_message();
                    current_length +=
                        sub_prompt.count_tokens_with_pregenerated_completion_message(&new_message, token_counter);

                    if let SubPromptType::UserLastMessage = prompt_type {
                        last_user_message = Some(new_message);
                    } else {
                        tiktoken_messages.push(new_message);
                    }
                }
                _ => {
                    // Process the current sub-prompt
                    let new_message = sub_prompt.into_chat_completion_request_message();
                    current_length +=
                        sub_prompt.count_tokens_with_pregenerated_completion_message(&new_message, token_counter);
                    tiktoken_messages.push(new_message);
                }
            }
        }

        // Combine ExtraContext and UserLastMessage into one message
        if !extra_context_content.is_empty() || last_user_message.is_some() {
            let combined_content = format!(
                "{}\n{}",
                extra_context_content.trim(),
                last_user_message
                    .as_ref()
                    .and_then(|msg| msg.content.clone())
                    .unwrap_or_default()
            )
            .trim()
            .to_string();

            let combined_message = LlmMessage {
                role: Some(SubPromptType::User.to_string()),
                content: Some(combined_content),
                name: None,
                function_call: None,
                functions: None,
                images: last_user_message.and_then(|msg| msg.images),
            };
            current_length += token_counter(&[combined_message.clone()]);
            tiktoken_messages.push(combined_message);
        }

        // Add function calls after the last user message
        for function_call in function_calls {
            tiktoken_messages.push(function_call);
        }

        // Add function call responses after function calls
        for response in function_call_responses {
            tiktoken_messages.push(response);
        }

        (tiktoken_messages, current_length)
    }

    /// Processes all sub-prompts into a single output String in OpenAI's message format.
    pub fn generate_openai_messages<F>(
        &self,
        max_prompt_tokens: Option<usize>,
        tool_response_field_name: Option<String>,
        token_counter: &F,
    ) -> Result<Vec<LlmMessage>, PromptError>
    where
        F: Fn(&[LlmMessage]) -> usize + Clone,
    {
        // We take about half of a default total 4097 if none is provided as a backup (should never happen)
        let limit = max_prompt_tokens.unwrap_or(2700_usize);

        // Remove sub-prompts until the total token count is under the specified limit
        let mut prompt_copy = self.clone();
        prompt_copy.remove_subprompts_until_under_max(limit, token_counter);

        // Generate the output chat completion request messages
        let (output_messages, _) =
            prompt_copy.generate_chat_completion_messages(tool_response_field_name, token_counter);

        Ok(output_messages)
    }

    // Generates generic api messages as a single string.
    //
    // TODO: needs to be updated
    //
    pub fn generate_genericapi_messages<F>(
        &self,
        max_input_tokens: Option<usize>,
        token_counter: &F,
    ) -> Result<String, PromptError>
    where
        F: Fn(&[LlmMessage]) -> usize,
    {
        let limit = max_input_tokens.unwrap_or(4000_usize);
        let mut prompt_copy = self.clone();
        prompt_copy.remove_subprompts_until_under_max(limit, token_counter);

        let mut messages: Vec<String> = Vec::new();
        // Process all sub-prompts in their original order
        for sub_prompt in prompt_copy.sub_prompts.iter() {
            match sub_prompt {
                SubPrompt::Asset(_, _, _, _, _) => {
                    // Ignore Asset
                }
                SubPrompt::Content(prompt_type, content, _priority_value) => {
                    let new_message = match prompt_type {
                        SubPromptType::System | SubPromptType::Assistant => format!("System: {}\n", content),
                        SubPromptType::User => format!("User: {}\n", content),
                        _ => String::new(),
                    };
                    if !new_message.is_empty() {
                        messages.push(new_message);
                    }
                }
                SubPrompt::FunctionCall(prompt_type, content, _priority_value)
                | SubPrompt::FunctionCallResponse(prompt_type, content, _priority_value) => {
                    let new_message = match prompt_type {
                        SubPromptType::System | SubPromptType::Assistant => format!("System: {}\n", content),
                        SubPromptType::User => format!("User: {}\n", content),
                        _ => String::new(),
                    };
                    if !new_message.is_empty() {
                        messages.push(new_message);
                    }
                }
                SubPrompt::ToolAvailable(_, content, _) => {
                    messages.push(format!("{}\n", content));
                }
                SubPrompt::Omni(_, _, _, _) => {
                    // Ignore Omni
                    // TODO: fix this
                }
            }
        }
        let output = messages.join(" ");
        // eprintln!("generate_genericapi_messages output: {:?}", output);
        Ok(output)
    }
}
