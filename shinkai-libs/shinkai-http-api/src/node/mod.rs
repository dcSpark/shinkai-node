use shinkai_message_primitives::schemas::shinkai_message::ShinkaiMessage;
use crate::error::APIError;

pub fn validate_message_main_logic(message: &ShinkaiMessage) -> Result<(), APIError> {
    // Basic validation logic
    if message.content.is_empty() {
        return Err(APIError::from("Message content cannot be empty"));
    }
    Ok(())
}

pub trait Node {
    fn process_message(&self, message: ShinkaiMessage) -> Result<(), APIError>;
}
