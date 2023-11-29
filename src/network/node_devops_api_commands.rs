use super::{
    node_api::{APIError, APIUseRegistrationCodeSuccessResponse},
    node_error::NodeError,
    Node,
};
use crate::{
    managers::identity_manager::{self, IdentityManager},
    network::node_message_handlers::{ping_pong, PingPong},
    planner::{kai_files::KaiJobFile, kai_manager::KaiJobFileManager},
    schemas::{
        identity::{DeviceIdentity, Identity, IdentityType, RegistrationCode, StandardIdentity, StandardIdentityType},
        inbox_permission::InboxPermission,
        smart_inbox::SmartInbox,
    },
};
use async_channel::Sender;
use blake3::Hasher;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use reqwest::StatusCode;
use shinkai_message_primitives::{
    schemas::shinkai_name::{ShinkaiName, ShinkaiNameError, ShinkaiSubidentityType},
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{
            APIAddAgentRequest, APIGetMessagesFromInboxRequest, APIReadUpToTimeRequest, IdentityPermissions,
            MessageSchemaType, RegistrationCodeRequest, RegistrationCodeType,
        },
    },
    shinkai_utils::{
        encryption::{
            clone_static_secret_key, encryption_public_key_to_string, encryption_secret_key_to_string,
            string_to_encryption_public_key, EncryptionMethod,
        },
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        signatures::{clone_signature_secret_key, signature_public_key_to_string, string_to_signature_public_key},
    },
};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl Node {
    pub async fn api_private_devops_cron_list(&self, res: Sender<Result<String, APIError>>) -> Result<(), NodeError> {
        // Call the get_all_cron_tasks_from_all_profiles function
        match self.db.lock().await.get_all_cron_tasks_from_all_profiles() {
            Ok(tasks) => {
                eprintln!("Got {} cron tasks", tasks.len());
                // If everything went well, send the tasks back as a JSON string
                let tasks_json = serde_json::to_string(&tasks).unwrap();
                let _ = res.send(Ok(tasks_json)).await;
                Ok(())
            }
            Err(err) => {
                // If there was an error, send the error message
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("{}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }
    }
}
