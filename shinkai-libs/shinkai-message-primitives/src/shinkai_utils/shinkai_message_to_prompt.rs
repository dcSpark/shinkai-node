use std::collections::HashMap;
use serde_json;
use crate::{
    schemas::{prompts::Prompt, subprompts::{SubPromptAssetType, SubPromptType}},
    shinkai_message::shinkai_message::{MessageBody, ShinkaiMessage},
    shinkai_message::shinkai_message_schemas::JobMessage,
};

impl ShinkaiMessage {
    pub fn to_prompt(&self) -> Prompt {
        let mut prompt = Prompt::new();

        // Access the recipient_subidentity from the internal metadata
        let recipient_subidentity = match &self.body {
            MessageBody::Unencrypted(body) => &body.internal_metadata.recipient_subidentity,
            _ => return prompt, // Return an empty prompt if the message is encrypted
        };

        // Deserialize the message content into a JobMessage
        let job_message: JobMessage = match serde_json::from_str(&self.get_message_content().unwrap_or_default()) {
            Ok(msg) => msg,
            Err(_) => return prompt, // Return an empty prompt if deserialization fails
        };

        // Determine the source of the message based on recipient_subidentity
        let sub_prompt_type = if recipient_subidentity == "main" {
            SubPromptType::Assistant
        } else {
            SubPromptType::User
        };

        // Add the job message content as an Omni sub-prompt
        prompt.add_omni(job_message.content, HashMap::new(), sub_prompt_type, 100);

        prompt
    }
}
