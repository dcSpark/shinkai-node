use crate::{
    schemas::{prompts::Prompt, subprompts::SubPromptType},
    shinkai_message::shinkai_message::{MessageBody, ShinkaiMessage},
    shinkai_message::shinkai_message_schemas::JobMessage,
};
use serde_json;
use std::collections::HashMap;

impl ShinkaiMessage {
    pub fn to_prompt(&self) -> Prompt {
        let mut prompt = Prompt::new();

        // Access the recipient_subidentity from the internal metadata
        let recipient_subidentity = match &self.body {
            MessageBody::Unencrypted(body) => &body.internal_metadata.recipient_subidentity,
            _ => {
                println!("Message is encrypted, returning empty prompt.");
                return prompt; // Return an empty prompt if the message is encrypted
            }
        };

        // Attempt to deserialize the message content into a JobMessage
        let job_message: JobMessage = match serde_json::from_str(&self.get_message_content().unwrap_or_default()) {
            Ok(msg) => msg,
            Err(_) => {
                JobMessage {
                    content: self.get_message_content().unwrap_or_default(),
                    job_id: "".to_string(),
                    parent: None,
                    sheet_job_data: None,
                    callback: None,
                    metadata: None,
                    tool_key: None,
                    fs_files_paths: vec![],
                job_filenames: vec![],
                }
            }
        };

        // Determine the source of the message based on recipient_subidentity
        let sub_prompt_type = if recipient_subidentity == "main" {
            SubPromptType::Assistant
        } else {
            SubPromptType::User
        };

        // Add the job message content as an Omni sub-prompt
        // println!("Adding omni sub-prompt with content: {}", job_message.content);
        prompt.add_omni(job_message.content, HashMap::new(), sub_prompt_type, 100);

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemas::{inbox_name::InboxName, subprompts::SubPrompt};
    use crate::shinkai_message::shinkai_message_schemas::MessageSchemaType;
    use crate::shinkai_utils::encryption::EncryptionMethod;
    use crate::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
    use ed25519_dalek::SigningKey;
    use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

    fn generate_shinkai_message(
        content: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        recipient_subidentity_name: String,
        origin_destination_identity_name: String,
        timestamp: String,
    ) -> ShinkaiMessage {
        let inbox_name = InboxName::get_job_inbox_name_from_params("test_job".to_string()).unwrap();

        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content(content.to_string())
            .body_encryption(EncryptionMethod::None)
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata_with_inbox(
                "".to_string(),
                recipient_subidentity_name.clone().to_string(),
                inbox_name_value,
                EncryptionMethod::None,
                None,
            )
            .external_metadata_with_schedule(
                origin_destination_identity_name.clone().to_string(),
                origin_destination_identity_name.clone().to_string(),
                timestamp,
            )
            .build()
            .unwrap()
    }

    #[test]
    fn test_to_prompt() {
        // Setup keys and other parameters
        let my_encryption_secret_key = EncryptionStaticKey::from([0u8; 32]);
        let my_signature_secret_key = SigningKey::from([0u8; 32]);
        let receiver_public_key = EncryptionPublicKey::from([0u8; 32]);
        let recipient_subidentity_name = "main_profile_node1".to_string();
        let origin_destination_identity_name = "@@node1.shinkai".to_string();
        let timestamp = "2023-07-02T20:53:34.811Z".to_string();

        // Generate the ShinkaiMessage using the helper function
        let message = generate_shinkai_message(
            "Hello World 1".to_string(),
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            recipient_subidentity_name,
            origin_destination_identity_name,
            timestamp,
        );

        let prompt = message.to_prompt();
        assert_eq!(prompt.sub_prompts.len(), 1);
        if let SubPrompt::Omni(_, content, _, _) = &prompt.sub_prompts[0] {
            assert_eq!(content, "Hello World 1");
        } else {
            panic!("Expected Omni variant");
        }
    }
}
