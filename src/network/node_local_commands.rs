use super::Node;
use crate::{
    network::node_api::APIError,
    schemas::{identity::Identity, inbox_permission::InboxPermission},
};
use async_channel::Sender;
use log::error;
use shinkai_message_primitives::{
    schemas::{
        agents::serialized_agent::SerializedAgent,
        shinkai_name::ShinkaiName,
    },
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{IdentityPermissions, RegistrationCodeType},
    },
};
use std::str::FromStr;
use crate::managers::identity_manager::IdentityManagerTrait;

impl Node {
    pub async fn local_get_last_unread_messages_from_inbox(
        &self,
        inbox_name: String,
        limit: usize,
        offset: Option<String>,
        res: Sender<Vec<ShinkaiMessage>>,
    ) {
        let result = self
            .internal_get_last_unread_messages_from_inbox(inbox_name, limit, offset)
            .await;
        if let Err(e) = res.send(result).await {
            error!("Failed to send last unread messages: {}", e);
        }
    }

    pub async fn local_get_last_messages_from_inbox(
        &self,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Vec<ShinkaiMessage>>,
    ) {
        // Query the database for the last `limit` number of messages from the specified inbox.
        let result = self
            .internal_get_last_messages_from_inbox(inbox_name, limit, offset_key)
            .await;

        let single_msg_array_array = result.into_iter().filter_map(|msg| msg.first().cloned()).collect();

        // Send the retrieved messages back to the requester.
        if let Err(e) = res.send(single_msg_array_array).await {
            error!("Failed to send last messages from inbox: {}", e);
        }
    }

    pub async fn local_get_last_messages_from_inbox_with_branches(
        &self,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Vec<Vec<ShinkaiMessage>>>,
    ) {
        // Query the database for the last `limit` number of messages from the specified inbox.
        let result = self
            .internal_get_last_messages_from_inbox(inbox_name, limit, offset_key)
            .await;

        // Send the retrieved messages back to the requester.
        if let Err(e) = res.send(result).await {
            error!("Failed to send last messages from inbox: {}", e);
        }
    }

    pub async fn local_mark_as_read_up_to(&self, inbox_name: String, up_to_time: String, res: Sender<String>) {
        // Attempt to mark messages as read in the database
        let result = self.internal_mark_as_read_up_to(inbox_name, up_to_time).await;

        // Convert the result to a string
        let result_str = match result {
            Ok(true) => "Marked as read successfully".to_string(),
            Ok(false) => "Failed to mark as read".to_string(),
            Err(e) => format!("Error: {}", e),
        };

        // Send the result back to the requester
        if let Err(e) = res.send(result_str).await {
            error!("Failed to send result: {}", e);
        }
    }

    pub async fn local_create_and_send_registration_code(
        &self,
        permissions: IdentityPermissions,
        code_type: RegistrationCodeType,
        res: Sender<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db = self.db.lock().await;
        let code = match db.generate_registration_new_code(permissions, code_type) {
            Ok(code) => code,
            Err(e) => {
                error!("Failed to generate registration new code: {}", e);
                "".to_string()
            }
        };
        if let Err(e) = res.send(code).await {
            error!("Failed to send code: {}", e);
            return Err(Box::new(e));
        }
        Ok(())
    }

    pub async fn local_get_all_subidentities_devices_and_agents(&self, res: Sender<Result<Vec<Identity>, APIError>>) {
        let identity_manager = self.identity_manager.lock().await;
        let result = identity_manager.get_all_subidentities_devices_and_agents();

        if let Err(e) = res.send(Ok(result)).await {
            error!("Failed to send result: {}", e);
            let error = APIError {
                code: 500,
                error: "ChannelSendError".to_string(),
                message: "Failed to send data through the channel".to_string(),
            };
            let _ = res.send(Err(error)).await;
        }
    }

    pub async fn local_add_inbox_permission(
        &self,
        inbox_name: String,
        perm_type: String,
        identity_name: String,
        res: Sender<String>,
    ) {
        // Obtain the IdentityManager and ShinkaiDB locks
        let identity_manager = self.identity_manager.lock().await;

        // Find the identity based on the provided name
        let identity = identity_manager.search_identity(&identity_name).await;

        // If identity is None (doesn't exist), return an error message
        if identity.is_none() {
            let _ = res
                .send(format!("No identity found with the name: {}", identity_name))
                .await;
            return;
        }

        let identity = identity.unwrap();

        // Check if the found identity is a StandardIdentity. If not, send an error message.
        let standard_identity = match &identity {
            Identity::Standard(std_identity) => std_identity.clone(),
            Identity::Device(_) => {
                // This case shouldn't happen because we are filtering out device identities
                let _ = res
                    .send(format!("Device identities cannot have inbox permissions"))
                    .await;
                return;
            }
            Identity::Agent(_) => {
                let _ = res
                    .send(format!("Agent identities cannot have inbox permissions"))
                    .await;
                return;
            }
        };

        let perm = InboxPermission::from_str(&perm_type).unwrap();
        let result = match self
            .db
            .lock()
            .await
            .add_permission(&inbox_name, &standard_identity, perm)
        {
            Ok(_) => "Success".to_string(),
            Err(e) => e.to_string(),
        };

        let _ = res.send(result);
    }

    pub async fn local_remove_inbox_permission(
        &self,
        inbox_name: String,
        _: String, // perm_type
        identity_name: String,
        res: Sender<String>,
    ) {
        // Obtain the IdentityManager and ShinkaiDB locks
        let identity_manager = self.identity_manager.lock().await;

        // Find the identity based on the provided name
        let identity = identity_manager.search_identity(&identity_name).await;

        // If identity is None (doesn't exist), return an error message
        if identity.is_none() {
            let _ = res
                .send(format!("No identity found with the name: {}", identity_name))
                .await;
            return;
        }

        let identity = identity.unwrap();

        // Check if the found identity is a StandardIdentity. If not, send an error message.
        let standard_identity = match &identity {
            Identity::Standard(std_identity) => std_identity.clone(),
            Identity::Device(std_device) => match std_device.clone().to_standard_identity() {
                Some(identity) => identity,
                None => {
                    let _ = res.send(format!("Device identity is not valid.")).await;
                    return;
                }
            },
            Identity::Agent(_) => {
                let _ = res
                    .send(format!("Agent identities cannot have inbox permissions"))
                    .await;
                return;
            }
        };

        // First, check if permission exists and remove it if it does
        match self.db.lock().await.remove_permission(&inbox_name, &standard_identity) {
            Ok(()) => {
                let _ = res
                    .send(format!(
                        "Permission removed successfully from identity {}.",
                        identity_name
                    ))
                    .await;
            }
            Err(e) => {
                let _ = res.send(format!("Error removing permission: {:?}", e)).await;
            }
        }
    }

    pub async fn local_create_new_job(&self, shinkai_message: ShinkaiMessage, res: Sender<(String, String)>) {
         let sender_name = match ShinkaiName::from_shinkai_message_using_sender_subidentity(&&shinkai_message.clone()) {
            Ok(name) => name,
            Err(e) => {
                error!("Failed to get sender name from message: {}", e);
                return;
            }
        };

        let subidentity_manager = self.identity_manager.lock().await;
        let sender_subidentity = subidentity_manager.find_by_identity_name(sender_name).cloned();
        std::mem::drop(subidentity_manager);

        let sender_subidentity = match sender_subidentity {
            Some(identity) => identity,
            None => {
                let _ = res.send((String::new(), "Sender subidentity not found".to_string())).await;
                return;
            }
        };

        match self.internal_create_new_job(shinkai_message, sender_subidentity).await {
            Ok(job_id) => {
                // If everything went well, send the job_id back with an empty string for error
                let _ = res.send((job_id, String::new())).await;
            }
            Err(err) => {
                // If there was an error, send the error message
                let _ = res.try_send((String::new(), format!("{}", err)));
            }
        };
    }

    // TODO: this interface changed. it's not returning job_id so the tuple is unnecessary
    pub async fn local_job_message(&self, shinkai_message: ShinkaiMessage, res: Sender<(String, String)>) {
        match self.internal_job_message(shinkai_message).await {
            Ok(_) => {
                // If everything went well, send the job_id back with an empty string for error
                let _ = res.send((String::new(), String::new())).await;
            }
            Err(err) => {
                // If there was an error, send the error message
                let _ = res.try_send((String::new(), format!("{}", err)));
            }
        };
    }

    pub async fn local_add_agent(&self, agent: SerializedAgent, profile: &ShinkaiName, res: Sender<String>) {
        let result = self.internal_add_agent(agent, profile).await;
        let result_str = match result {
            Ok(_) => "true".to_string(),
            Err(e) => format!("Error: {:?}", e),
        };
        let _ = res.send(result_str).await;
    }

    pub async fn local_available_agents(
        &self,
        full_profile_name: String,
        res: Sender<Result<Vec<SerializedAgent>, String>>,
    ) {
        match self.internal_get_agents_for_profile(full_profile_name).await {
            Ok(agents) => {
                let _ = res.send(Ok(agents)).await;
            }
            Err(err) => {
                let _ = res.send(Err(format!("Internal Server Error: {}", err))).await;
            }
        }
    }

    pub async fn local_is_pristine(&self, res: Sender<bool>) {
        let db_lock = self.db.lock().await;
        let has_any_profile = db_lock.has_any_profile().unwrap_or(false);
        let _ = res.send(!has_any_profile).await;
    }

    pub async fn local_scan_ollama_models(&self, res: Sender<Result<Vec<String>, String>>) {
        let result = self.internal_scan_ollama_models().await;
        let _ = res.send(result.map_err(|e| e.message)).await;
    }

    pub async fn local_add_ollama_models(&self, input_models: Vec<String>, res: Sender<Result<(), String>>) {
        let result = self.internal_add_ollama_models(input_models).await;
        let _ = res.send(result).await;
    }
}
