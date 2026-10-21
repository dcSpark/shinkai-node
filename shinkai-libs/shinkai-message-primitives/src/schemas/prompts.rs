use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shinkai_message::shinkai_message::ShinkaiMessage;

use super::{
    llm_message::{DetailedFunctionCall, LlmMessage},
    shinkai_fs::ShinkaiFileChunkCollection,
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
    /// The lowest priority value held in sub_prompts. TODO: Make this a hashmap to make it more efficient for updating
    /// priorities.
    pub lowest_priority: u8,
    /// The highest priority value held in sub_prompts. TODO: Make this a hashmap to make it more efficient for
    /// updating priorities.
    pub highest_priority: u8,
}

// Metadata contains a complete provider turn, repeated on each parallel call.
// Retain the positions of its calls and results so truncation is atomic.
struct NativeToolTurn<'a> {
    positions: Vec<usize>,
    call_ids: HashSet<String>,
    payloads: Vec<(&'static str, &'a Value)>,
}

fn native_replay_payload(metadata: Option<&Value>) -> Option<(&'static str, &Value)> {
    let metadata = metadata?;
    for key in ["openrouter_assistant", "claude_content", "gemini_parts"] {
        if let Some(value) = metadata.get(key).filter(|value| value.is_object() || value.is_array()) {
            return Some((key, value));
        }
    }
    if let Some(value) = metadata
        .pointer("/openai_responses/output")
        .filter(|value| value.is_array())
    {
        return Some(("openai_responses", value));
    }
    if let Some(value) = metadata
        .get("deepseek_reasoning_content")
        .filter(|value| value.is_string())
    {
        return Some(("deepseek_reasoning_content", value));
    }
    metadata
        .get("reasoning_details")
        .filter(|value| value.is_array())
        .map(|value| ("reasoning_details", value))
}

enum ToolHistoryItem<'a> {
    Call {
        metadata: Option<&'a Value>,
        id: Option<&'a str>,
        name: Option<&'a str>,
    },
    Result {
        metadata: Option<&'a Value>,
        id: Option<&'a str>,
        name: Option<&'a str>,
    },
    Boundary,
}

fn native_tool_turns<'a>(items: impl Iterator<Item = ToolHistoryItem<'a>>) -> Vec<NativeToolTurn<'a>> {
    let mut turns: Vec<NativeToolTurn<'a>> = Vec::new();
    let mut tagged_turns = HashMap::<&str, usize>::new();
    let mut call_owners = HashMap::<(bool, &str), Option<usize>>::new();
    let mut legacy_turn: Option<usize> = None;
    for (position, item) in items.enumerate() {
        match item {
            ToolHistoryItem::Call {
                metadata,
                id: call_id,
                name,
            } => {
                let payload = native_replay_payload(metadata);
                let tag = metadata
                    .and_then(|value| value.get("inference_turn_id"))
                    .and_then(Value::as_str);
                let existing = match tag {
                    Some(tag) => {
                        legacy_turn = None;
                        tagged_turns.get(tag).copied()
                    }
                    None => legacy_turn.filter(|&index| {
                        payload.is_some()
                            && turns[index].payloads.first().copied() == payload
                            && !call_id.map(|id| turns[index].call_ids.contains(id)).unwrap_or(false)
                    }),
                };
                if payload.is_none() && tag.is_none() {
                    legacy_turn = None;
                    if let Some(id) = call_id {
                        call_owners.insert((true, id), None);
                    }
                    if let Some(name) = name {
                        call_owners.insert((false, name), None);
                    }
                    continue;
                }
                let index = existing.unwrap_or_else(|| {
                    turns.push(NativeToolTurn {
                        positions: vec![],
                        call_ids: HashSet::new(),
                        payloads: vec![],
                    });
                    turns.len() - 1
                });
                if let Some(tag) = tag {
                    tagged_turns.insert(tag, index);
                } else {
                    legacy_turn = Some(index);
                }
                let turn = &mut turns[index];
                turn.positions.push(position);
                if let Some(id) = call_id {
                    turn.call_ids.insert(id.into());
                }
                if let Some(id) = call_id {
                    call_owners.insert((true, id), Some(index));
                }
                if let Some(name) = name {
                    call_owners.insert((false, name), Some(index));
                }
                if let Some(payload) = payload {
                    if !turn.payloads.contains(&payload) {
                        turn.payloads.push(payload);
                    }
                }
            }
            ToolHistoryItem::Result { metadata, id, name } => {
                let tag = metadata
                    .and_then(|value| value.get("inference_turn_id"))
                    .and_then(Value::as_str);
                let index = tag.and_then(|tag| tagged_turns.get(tag).copied()).or_else(|| {
                    id.and_then(|id| call_owners.get(&(true, id)).copied().flatten())
                        .or_else(|| name.and_then(|name| call_owners.get(&(false, name)).copied().flatten()))
                });
                if let Some(index) = index {
                    turns[index].positions.push(position);
                }
            }
            ToolHistoryItem::Boundary => legacy_turn = None,
        }
    }
    turns.retain(|turn| !turn.payloads.is_empty());
    turns
}

impl Default for Prompt {
    fn default() -> Self {
        Self::new()
    }
}

impl Prompt {
    fn native_tool_turns(&self) -> Vec<NativeToolTurn<'_>> {
        native_tool_turns(self.sub_prompts.iter().map(|item| match item {
            SubPrompt::FunctionCall(_, call, _) => ToolHistoryItem::Call {
                metadata: call.get("provider_metadata"),
                id: call.get("call_id").and_then(Value::as_str),
                name: call.get("name").and_then(Value::as_str),
            },
            SubPrompt::FunctionCallResponse(_, response, _) => {
                let call = &response["function_call"];
                ToolHistoryItem::Result {
                    metadata: call.get("provider_metadata"),
                    id: call.get("id").and_then(Value::as_str),
                    name: call.get("name").and_then(Value::as_str),
                }
            }
            _ => ToolHistoryItem::Boundary,
        }))
    }

    /// Conservative token charge for native payloads that adapters replay.
    /// Parallel calls share a turn; separate inference turns are charged separately.
    pub fn count_native_replay_tokens(messages: &[LlmMessage]) -> usize {
        let turns = native_tool_turns(messages.iter().map(|message| {
            if let Some(call) = &message.function_call {
                ToolHistoryItem::Call {
                    metadata: call.provider_metadata.as_ref(),
                    id: call.id.as_deref(),
                    name: Some(&call.name),
                }
            } else if matches!(message.role.as_deref(), Some("function" | "tool")) {
                ToolHistoryItem::Result {
                    metadata: None,
                    id: message.name.as_deref(),
                    name: message.name.as_deref(),
                }
            } else {
                ToolHistoryItem::Boundary
            }
        }));
        turns
            .into_iter()
            .flat_map(|turn| turn.payloads)
            .fold(0usize, |total, (_, payload)| {
                total.saturating_add(payload.to_string().len())
            })
    }

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
            .map(|(file_name, file_content)| {
                let asset_type = Self::detect_asset_type(&file_name);
                (asset_type, file_content, file_name)
            })
            .collect();
        let sub_prompt = SubPrompt::Omni(prompt_type, content, assets, capped_priority_value as u8);
        self.add_sub_prompt(sub_prompt);
    }

    /// Detects the asset type based on file extension
    fn detect_asset_type(file_name: &str) -> SubPromptAssetType {
        let file_name_lower = file_name.to_lowercase();

        // Check for image extensions
        if file_name_lower.ends_with(".png")
            || file_name_lower.ends_with(".jpg")
            || file_name_lower.ends_with(".jpeg")
            || file_name_lower.ends_with(".gif")
            || file_name_lower.ends_with(".bmp")
            || file_name_lower.ends_with(".webp")
        {
            return SubPromptAssetType::Image;
        }

        // Check for video extensions
        if file_name_lower.ends_with(".mp4")
            || file_name_lower.ends_with(".mov")
            || file_name_lower.ends_with(".avi")
            || file_name_lower.ends_with(".webm")
            || file_name_lower.ends_with(".mkv")
            || file_name_lower.ends_with(".wmv")
            || file_name_lower.ends_with(".flv")
        {
            return SubPromptAssetType::Video;
        }

        // Check for audio extensions
        if file_name_lower.ends_with(".mp3")
            || file_name_lower.ends_with(".wav")
            || file_name_lower.ends_with(".flac")
            || file_name_lower.ends_with(".ogg")
            || file_name_lower.ends_with(".m4a")
            || file_name_lower.ends_with(".aiff")
            || file_name_lower.ends_with(".wma")
            || file_name_lower.ends_with(".aac")
        {
            return SubPromptAssetType::Audio;
        }

        // Default to Image for unknown types (backward compatibility)
        SubPromptAssetType::Image
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
        retrieved_node: ShinkaiFileChunkCollection,
        prompt_type: SubPromptType,
        priority_value: u8,
    ) {
        for chunk in retrieved_node.chunks.iter() {
            let content = chunk.content.clone();
            if !content.trim().is_empty() {
                self.add_content(content, prompt_type.clone(), priority_value);
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

    /// Adds previous results from step history into the Prompt, up to max_tokens.
    /// Note: The last message in the history is not added.
    /// Of note, priority value must be between 0-100.
    pub fn add_step_history(&mut self, history: Vec<ShinkaiMessage>, priority_value: u8) {
        let capped_priority_value = std::cmp::min(priority_value, 100) as u8;
        let sub_prompts_list: Vec<SubPrompt> = history
            .iter()
            .take(history.len().saturating_sub(1)) // Skip the last message
            .filter_map(|step| Some(step.to_prompt()))
            .flat_map(|prompt| prompt.sub_prompts.clone())
            .collect();
        self.add_sub_prompts_with_new_priority(sub_prompts_list, capped_priority_value);
    }

    /// Removes the first sub-prompt from the end of the sub_prompts list that has the lowest priority value.
    /// Used primarily for cutting down prompt when it is too large to fit in context window.
    pub fn remove_lowest_priority_sub_prompt(&mut self) -> Option<SubPrompt> {
        self.lowest_priority_position()
            .map(|position| self.remove_sub_prompt(position))
    }

    fn lowest_priority_position(&self) -> Option<usize> {
        let lowest_priority = self.lowest_priority;
        self.sub_prompts.iter().rposition(|sub_prompt| match sub_prompt {
            SubPrompt::Content(_, _, priority) | SubPrompt::ToolAvailable(_, _, priority) => {
                *priority == lowest_priority
            }
            SubPrompt::Asset(_, _, _, _, priority) => *priority == lowest_priority,
            SubPrompt::FunctionCall(_, _, priority) => *priority == lowest_priority,
            SubPrompt::FunctionCallResponse(_, _, priority) => *priority == lowest_priority,
            SubPrompt::Omni(_, _, _, priority) => *priority == lowest_priority,
        })
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
        while current_token_count.saturating_add(200) > max_prompt_tokens {
            let Some(position) = self.lowest_priority_position() else {
                break;
            };
            let positions = self
                .native_tool_turns()
                .into_iter()
                .find(|turn| turn.positions.contains(&position))
                .map(|turn| turn.positions)
                .unwrap_or_else(|| vec![position]);
            for position in positions.into_iter().rev() {
                removed_subprompts.push(self.remove_sub_prompt(position));
            }
            // Native metadata is shared by calls. Subtracting one raw sub-prompt
            // cannot represent the cost of the retained provider payload.
            current_token_count = self.generate_chat_completion_messages(None, token_counter).1;
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
        let mut tool_messages: Vec<LlmMessage> = Vec::new();

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
                        videos: None,
                        audios: None,
                        tool_calls: None,
                    };

                    if let Some(name) = content.get("name").and_then(|n| n.as_str()) {
                        let arguments = content
                            .get("arguments")
                            .map_or_else(|| "".to_string(), |args| args.to_string());

                        let call_id = content
                            .get("call_id")
                            .and_then(|id| id.as_str())
                            .unwrap_or("")
                            .to_string();

                        new_message.function_call = Some(DetailedFunctionCall {
                            provider_metadata: content.get("provider_metadata").cloned().filter(|v| !v.is_null()),
                            name: name.to_string(),
                            arguments,
                            id: if call_id.is_empty() { None } else { Some(call_id) },
                        });
                    }
                    // Count generic arguments as before. Native metadata is
                    // accounted once per turn below, even if this counter also
                    // understands native message history.
                    let metadata = new_message
                        .function_call
                        .as_mut()
                        .and_then(|call| call.provider_metadata.take());
                    current_length +=
                        sub_prompt.count_tokens_with_pregenerated_completion_message(&new_message, token_counter);
                    if let Some(call) = new_message.function_call.as_mut() {
                        call.provider_metadata = metadata;
                    }
                    tool_messages.push(new_message);
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
                        videos: None,
                        audios: None,
                        tool_calls: None,
                    };

                    if let Some(function_call) = content.get("function_call") {
                        if let Some(name) = function_call.get("name").and_then(|n| n.as_str()) {
                            new_message.name = Some(name.to_string());
                        }

                        // Hack to make the tool call id available to the LLM if id is present
                        if let Some(call_id) = function_call.get("id").and_then(|c| c.as_str()) {
                            new_message.name = Some(call_id.to_string());
                        }
                    }
                    new_message.content = content.get("response").and_then(|r| r.as_str()).map(|r| r.to_string());

                    current_length +=
                        sub_prompt.count_tokens_with_pregenerated_completion_message(&new_message, token_counter);
                    tool_messages.push(new_message);
                }
                SubPrompt::Content(SubPromptType::UserLastMessage, content, _) => {
                    last_user_message = Some(LlmMessage {
                        role: Some(SubPromptType::User.to_string()),
                        content: Some(content.clone()),
                        name: None,
                        function_call: None,
                        functions: None,
                        images: None,
                        videos: None,
                        audios: None,
                        tool_calls: None,
                    });
                }
                SubPrompt::Omni(prompt_type, _, _, _) => {
                    // Process the current sub-prompt
                    let new_message = sub_prompt.into_chat_completion_request_message();

                    if let SubPromptType::UserLastMessage = prompt_type {
                        last_user_message = Some(new_message);
                    } else {
                        current_length +=
                            sub_prompt.count_tokens_with_pregenerated_completion_message(&new_message, token_counter);
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
                images: last_user_message.as_ref().and_then(|msg| msg.images.clone()),
                videos: last_user_message.as_ref().and_then(|msg| msg.videos.clone()),
                audios: last_user_message.as_ref().and_then(|msg| msg.audios.clone()),
                tool_calls: None,
            };
            current_length += token_counter(&[combined_message.clone()]);
            tiktoken_messages.push(combined_message);
        }

        // Keep tool turns in their original order after the current user message.
        tiktoken_messages.extend(tool_messages);

        for turn in self.native_tool_turns() {
            for (_, payload) in turn.payloads {
                let encoded = payload.to_string();
                let bytes = encoded.len();
                let message = LlmMessage {
                    role: Some("assistant".into()),
                    content: Some(encoded),
                    ..Default::default()
                };
                // Opaque signatures and encrypted reasoning can tokenize very
                // differently from prose. One token per encoded byte is a
                // conservative bound, without changing the stored native data.
                current_length = current_length.saturating_add(bytes.max(token_counter(&[message])));
            }
        }

        (tiktoken_messages, current_length)
    }

    /// Processes all sub-prompts into a single output String in OpenAI's message format.
    pub fn generate_llm_messages<F>(
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

#[cfg(test)]
mod provider_history_tests {
    use super::*;
    use serde_json::json;

    fn text_and_call_tokens(messages: &[LlmMessage]) -> usize {
        messages
            .iter()
            .map(|message| {
                message.content.as_ref().map_or(0, String::len)
                    + message
                        .function_call
                        .as_ref()
                        .map_or(0, |call| call.name.len() + call.arguments.len())
            })
            .sum()
    }

    fn add_turn(prompt: &mut Prompt, metadata: Value, ids: &[&str], priority: u8) {
        for id in ids {
            prompt.add_function_call(json!({"name":"tool", "call_id":id, "arguments":{"query":"keep arguments"}, "provider_metadata":metadata}), priority);
        }
        for id in ids {
            prompt.add_function_call_response(
                json!({"function_call":{"name":"tool", "id":id, "provider_metadata":metadata}, "response":"done"}),
                priority,
            );
        }
    }

    fn without_metadata(prompt: &Prompt) -> Prompt {
        let mut prompt = prompt.clone();
        for item in &mut prompt.sub_prompts {
            if let SubPrompt::FunctionCall(_, call, _) = item {
                call.as_object_mut().unwrap().remove("provider_metadata");
            }
        }
        prompt
    }

    #[test]
    fn native_payloads_reduce_the_remaining_budget() {
        let opaque = "signed reasoning ".repeat(1024);
        for (metadata, payload) in [
            (
                json!({"claude_content":[{"type":"thinking","thinking":opaque,"signature":"opaque signature"}]}),
                json!([{"type":"thinking","thinking":opaque,"signature":"opaque signature"}]),
            ),
            (
                json!({"gemini_parts":[{"text":opaque,"thoughtSignature":"opaque signature"}]}),
                json!([{"text":opaque,"thoughtSignature":"opaque signature"}]),
            ),
            (
                json!({"openai_responses":{"output":[{"type":"reasoning","encrypted_content":opaque}]}}),
                json!([{"type":"reasoning","encrypted_content":opaque}]),
            ),
            (
                json!({"openrouter_assistant":{"role":"assistant","tool_calls":[],"reasoning_details":[{"data":opaque}]},"reasoning_details":[{"data":opaque}]}),
                json!({"role":"assistant","tool_calls":[],"reasoning_details":[{"data":opaque}]}),
            ),
            (json!({"deepseek_reasoning_content":opaque}), json!(opaque)),
            (json!({"reasoning_details":[{"data":opaque}]}), json!([{"data":opaque}])),
        ] {
            let mut prompt = Prompt::new();
            add_turn(&mut prompt, metadata, &["a"], 10);
            let baseline = without_metadata(&prompt)
                .generate_chat_completion_messages(None, &text_and_call_tokens)
                .1;
            let actual = prompt.generate_chat_completion_messages(None, &text_and_call_tokens).1;
            assert_eq!(actual - baseline, payload.to_string().len());
            let budget = actual + 200;
            assert!(budget.saturating_sub(actual) < budget.saturating_sub(baseline));
        }
    }

    #[test]
    fn parallel_native_metadata_is_charged_once_per_turn() {
        let payload = json!([{"type":"thinking","thinking":"long reasoning ".repeat(500),"signature":"signed"}]);
        let mut prompt = Prompt::new();
        add_turn(
            &mut prompt,
            json!({"inference_turn_id":"first","claude_content":payload}),
            &["a", "b"],
            10,
        );
        let baseline = without_metadata(&prompt)
            .generate_chat_completion_messages(None, &text_and_call_tokens)
            .1;
        assert_eq!(
            prompt.generate_chat_completion_messages(None, &text_and_call_tokens).1 - baseline,
            payload.to_string().len()
        );
        // An identical payload in a later actual inference turn is not a clone.
        add_turn(
            &mut prompt,
            json!({"inference_turn_id":"second","claude_content":payload}),
            &["c"],
            10,
        );
        let baseline = without_metadata(&prompt)
            .generate_chat_completion_messages(None, &text_and_call_tokens)
            .1;
        assert_eq!(
            prompt.generate_chat_completion_messages(None, &text_and_call_tokens).1 - baseline,
            2 * payload.to_string().len()
        );
    }

    #[test]
    fn untagged_parallel_history_uses_local_turn_boundaries() {
        let payload = json!([{"thoughtSignature":"signed","text":"reasoning ".repeat(100)}]);
        let metadata = json!({"gemini_parts":payload});
        let mut prompt = Prompt::new();
        // Older stored prompts alternate individual calls and their results.
        add_turn(&mut prompt, metadata.clone(), &["a"], 10);
        add_turn(&mut prompt, metadata.clone(), &["b"], 10);
        let baseline = without_metadata(&prompt)
            .generate_chat_completion_messages(None, &text_and_call_tokens)
            .1;
        assert_eq!(
            prompt.generate_chat_completion_messages(None, &text_and_call_tokens).1 - baseline,
            payload.to_string().len()
        );
        prompt.add_content("A later conversation turn".into(), SubPromptType::Assistant, 10);
        add_turn(&mut prompt, metadata, &["c"], 10);
        let baseline = without_metadata(&prompt)
            .generate_chat_completion_messages(None, &text_and_call_tokens)
            .1;
        assert_eq!(
            prompt.generate_chat_completion_messages(None, &text_and_call_tokens).1 - baseline,
            2 * payload.to_string().len()
        );
    }

    #[test]
    fn native_truncation_removes_complete_turns_and_recounts() {
        let payload = json!([{"type":"thinking","thinking":"long reasoning ".repeat(500),"signature":"must remain exact"}, {"type":"tool_use","id":"a","name":"tool","input":{}}]);
        let metadata = json!({"inference_turn_id":"first","claude_content":payload});
        let mut prompt = Prompt::new();
        prompt.add_content("Keep the current request".into(), SubPromptType::UserLastMessage, 100);
        prompt.add_content("Remove this context first".repeat(100), SubPromptType::ExtraContext, 1);
        add_turn(&mut prompt, metadata.clone(), &["a", "b"], 10);
        let before = prompt.generate_chat_completion_messages(None, &text_and_call_tokens).1;
        // Enough space preserves signed native data byte-for-byte.
        let messages = prompt
            .generate_llm_messages(Some(before + 200), None, &text_and_call_tokens)
            .unwrap();
        for message in messages.iter().filter_map(|message| message.function_call.as_ref()) {
            assert_eq!(message.provider_metadata.as_ref(), Some(&metadata));
        }
        let removed = prompt.remove_subprompts_until_under_max(300, &text_and_call_tokens);
        assert_eq!(removed.len(), 5); // Context plus both calls and both results.
        assert_eq!(prompt.sub_prompts.len(), 1);
        assert!(prompt.generate_chat_completion_messages(None, &text_and_call_tokens).1 + 200 <= 300);
        assert_eq!(
            removed
                .iter()
                .filter(|item| matches!(item, SubPrompt::FunctionCall(_, _, _)))
                .count(),
            2
        );
        assert_eq!(
            removed
                .iter()
                .filter(|item| matches!(item, SubPrompt::FunctionCallResponse(_, _, _)))
                .count(),
            2
        );
    }

    #[test]
    fn metadata_free_calls_keep_existing_argument_accounting() {
        let mut prompt = Prompt::new();
        add_turn(&mut prompt, Value::Null, &["a", "b"], 10);
        let expected = prompt.generate_chat_completion_messages(None, &text_and_call_tokens).1;
        let mut tagged = prompt.clone();
        for item in &mut tagged.sub_prompts {
            if let SubPrompt::FunctionCall(_, call, _) = item {
                call["provider_metadata"] = json!({"inference_turn_id":"generic"});
            }
        }
        assert_eq!(
            tagged.generate_chat_completion_messages(None, &text_and_call_tokens).1,
            expected
        );
        assert_eq!(
            without_metadata(&prompt)
                .generate_chat_completion_messages(None, &text_and_call_tokens)
                .1,
            expected
        );
    }

    #[test]
    fn test_provider_metadata_prompt_round_trip() {
        let mut prompt = Prompt::new();
        prompt.add_content("Use two tools".into(), SubPromptType::UserLastMessage, 100);
        for (name, signature) in [("first", "signed-one"), ("second", "signed-two")] {
            prompt.add_function_call(
                json!({
                    "name": name, "call_id": name, "arguments": {},
                    "provider_metadata": {"gemini_parts": [{"thoughtSignature": signature}]}
                }),
                100,
            );
            prompt.add_function_call_response(
                json!({
                    "function_call": {"name": name, "id": name}, "response": "done"
                }),
                100,
            );
        }
        let saved = serde_json::to_string(&prompt).unwrap();
        let loaded: Prompt = serde_json::from_str(&saved).unwrap();
        let (messages, _) = loaded.generate_chat_completion_messages(None, &|_| 0);
        assert_eq!(
            messages.iter().map(|m| m.role.as_deref().unwrap()).collect::<Vec<_>>(),
            ["user", "assistant", "function", "assistant", "function"]
        );
        assert_eq!(
            messages[1]
                .function_call
                .as_ref()
                .unwrap()
                .provider_metadata
                .as_ref()
                .unwrap()["gemini_parts"][0]["thoughtSignature"],
            "signed-one"
        );
        assert_eq!(
            messages[3]
                .function_call
                .as_ref()
                .unwrap()
                .provider_metadata
                .as_ref()
                .unwrap()["gemini_parts"][0]["thoughtSignature"],
            "signed-two"
        );
    }

    #[test]
    fn test_old_function_history_has_no_provider_metadata() {
        let call: DetailedFunctionCall = serde_json::from_value(json!({
            "name": "legacy", "arguments": "{}", "id": "old-id"
        }))
        .unwrap();
        assert!(call.provider_metadata.is_none());
        assert!(serde_json::to_value(call).unwrap().get("provider_metadata").is_none());
    }
}
