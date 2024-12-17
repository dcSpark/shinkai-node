use shinkai_message_primitives::shinkai_message::shinkai_message::{MessageBody, ShinkaiMessage};
use shinkai_message_primitives::schemas::identity::Identity;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;
use crate::{error::APIError, identity::IdentityManagerTrait};

pub fn validate_message_main_logic(
    message: &ShinkaiMessage,
    _identity_manager: Arc<tokio::sync::Mutex<dyn IdentityManagerTrait + Send>>,
    _sender_name: &ShinkaiName,
    _original_message: ShinkaiMessage,
    _parent_key: Option<String>,
) -> Result<(), APIError> {
    match &message.body {
        MessageBody::Encrypted(_) | MessageBody::Unencrypted(_) => Ok(()),
    }
}

#[async_trait::async_trait]
pub trait Node: Send + Sync {
    async fn has_inbox_access(
        db: Arc<SqliteManager>,
        inbox: &InboxName,
        sender: &Identity,
    ) -> Result<bool, APIError>;
}
