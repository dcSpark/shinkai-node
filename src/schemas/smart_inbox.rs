use serde::{Deserialize, Serialize};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SmartInbox {
    pub inbox_id: String,
    pub custom_name: String,
    pub last_message: Option<ShinkaiMessage>,
}
