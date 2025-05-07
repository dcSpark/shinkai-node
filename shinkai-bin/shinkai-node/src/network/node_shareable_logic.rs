use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::identity::{Identity, StandardIdentityType};
use std::io::Read;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::bytes::Bytes;

use crate::managers::identity_manager::IdentityManager;
use crate::managers::identity_manager::IdentityManagerTrait;
use ed25519_dalek::{ed25519::signature::SignerMut, SigningKey};
use hex;
use log::error;
use reqwest::StatusCode;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::MessageSchemaType},
    shinkai_utils::encryption::string_to_encryption_public_key,
};
use x25519_dalek::StaticSecret as EncryptionStaticKey;

pub async fn validate_message_main_logic(
    encryption_secret_key: &EncryptionStaticKey,
    identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
    node_profile_name: &ShinkaiName,
    potentially_encrypted_msg: ShinkaiMessage,
    schema_type: Option<MessageSchemaType>,
) -> Result<(ShinkaiMessage, Identity), APIError> {
    let msg: ShinkaiMessage;
    {
        // check if the message is encrypted
        let is_body_encrypted = potentially_encrypted_msg.clone().is_body_currently_encrypted();
        if is_body_encrypted {
            /*
            When someone sends an encrypted message, we need to compute the shared key using Diffie-Hellman,
            but what if they are using a subidentity? We don't know which one because it's encrypted.
            So the only way to get the pk is if they send it to us in the external_metadata.other field or
            if they are using intra_sender (which needs to be deleted afterwards).
            For other cases, we can find it in the identity manager.
            */
            let sender_encryption_pk_string = potentially_encrypted_msg.external_metadata.clone().other;
            let sender_encryption_pk = string_to_encryption_public_key(sender_encryption_pk_string.as_str()).ok();

            if sender_encryption_pk.is_some() {
                msg = match potentially_encrypted_msg
                    .clone()
                    .decrypt_outer_layer(encryption_secret_key, &sender_encryption_pk.unwrap())
                {
                    Ok(msg) => msg,
                    Err(e) => {
                        return Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: format!("Failed to decrypt message body: {}", e),
                        })
                    }
                };
            } else {
                let sender_name = ShinkaiName::from_shinkai_message_using_sender_and_intra_sender(
                    &potentially_encrypted_msg.clone(),
                )?;

                let sender_encryption_pk = match identity_manager
                    .lock()
                    .await
                    .search_identity(sender_name.clone().to_string().as_str())
                    .await
                {
                    Some(identity) => match identity {
                        Identity::Standard(std_identity) => match std_identity.identity_type {
                            StandardIdentityType::Global => std_identity.node_encryption_public_key,
                            StandardIdentityType::Profile => std_identity
                                .profile_encryption_public_key
                                .unwrap_or(std_identity.node_encryption_public_key),
                        },
                        Identity::Device(device) => device.device_encryption_public_key,
                        Identity::LLMProvider(_) => {
                            return Err(APIError {
                                code: StatusCode::UNAUTHORIZED.as_u16(),
                                error: "Unauthorized".to_string(),
                                message:
                                    "Failed to get sender encryption pk from message: Agent identity not supported"
                                        .to_string(),
                            })
                        }
                    },
                    None => {
                        return Err(APIError {
                            code: StatusCode::UNAUTHORIZED.as_u16(),
                            error: "Unauthorized".to_string(),
                            message: "Failed to get sender encryption pk from message: Identity not found".to_string(),
                        })
                    }
                };
                msg = match potentially_encrypted_msg
                    .clone()
                    .decrypt_outer_layer(encryption_secret_key, &sender_encryption_pk)
                {
                    Ok(msg) => msg,
                    Err(e) => {
                        return Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: format!("Failed to decrypt message body: {}", e),
                        })
                    }
                };
            }
        } else {
            msg = potentially_encrypted_msg.clone();
        }
    }

    // shinkai_log(
    //     ShinkaiLogOption::Identity,
    //     ShinkaiLogLevel::Info,
    //     format!("after decrypt_message_body_if_needed: {:?}", msg).as_str(),
    // );

    // Check that the message has the right schema type
    if let Some(schema) = schema_type {
        if let Err(e) = msg.validate_message_schema(schema) {
            return Err(APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Invalid message schema: {}", e),
            });
        }
    }

    // Check if the message is coming from one of our subidentities and validate signature
    let sender_name = match ShinkaiName::from_shinkai_message_using_sender_subidentity(&msg.clone()) {
        Ok(name) => name,
        Err(e) => {
            return Err(APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Failed to get sender name from message: {}", e),
            })
        }
    };

    // We (currently) don't proxy external messages from other nodes to other nodes
    if sender_name.get_node_name_string() != node_profile_name.get_node_name_string() {
        return Err(APIError {
            code: StatusCode::BAD_REQUEST.as_u16(),
            error: "Bad Request".to_string(),
            message: "sender_name.node_name is not the same as self.node_name. It can't proxy through this node."
                .to_string(),
        });
    }

    // Check that the subidentity that's trying to prox through us exist / is valid and linked to the node
    let subidentity_manager = identity_manager.lock().await;
    let sender_subidentity = subidentity_manager.find_by_identity_name(sender_name).cloned();
    std::mem::drop(subidentity_manager);

    // eprintln!(
    //     "\n\nafter find_by_identity_name> sender_subidentity: {:?}",
    //     sender_subidentity
    // );

    // Check that the identity exists locally
    let sender_subidentity = match sender_subidentity.clone() {
        Some(sender) => sender,
        None => {
            return Err(APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Sender subidentity is None".to_string(),
            });
        }
    };

    // Check that the message signature is valid according to the local keys
    match IdentityManager::verify_message_signature(
        Some(sender_subidentity.clone()),
        &potentially_encrypted_msg,
        &msg.clone(),
    ) {
        Ok(_) => (),
        Err(e) => {
            error!("Failed to verify message signature: {}", e);
            return Err(APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Failed to verify message signature: {}", e),
            });
        }
    }

    Ok((msg, sender_subidentity))
}

pub struct ZipFileContents {
    pub buffer: Vec<u8>,
    pub archive: zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
}

pub async fn download_zip_from_url(
    url: String,
    file_name: String,
    node_name: String,
    signing_secret_key: SigningKey,
) -> Result<ZipFileContents, APIError> {
    // Signature
    let signature = signing_secret_key
        .clone()
        .try_sign(url.as_bytes())
        .map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed to sign tool: {}", e),
        })?;

    let signature_bytes = signature.to_bytes();
    let signature_hex = hex::encode(signature_bytes);

    // Create the request with headers
    let client = reqwest::Client::new();
    let request = client
        .get(&url)
        .header("X-Shinkai-Version", env!("CARGO_PKG_VERSION"))
        .header("X-Shinkai-Identity", node_name)
        .header("X-Shinkai-Signature", signature_hex);

    // Send the request
    let response = match request.send().await {
        Ok(response) => response,
        Err(err) => {
            return Err(APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Download Failed".to_string(),
                message: format!("Failed to download asset from URL: {}", err),
            });
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        println!("Download failed with status: {}", status);
        println!("Response body: {}", body);
        return Err(APIError {
            code: status.as_u16(),
            error: "Download Failed".to_string(),
            message: format!("Failed to download asset from URL: {}", status),
        });
    }

    // Get the bytes from the response
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Download Failed".to_string(),
                message: format!("Failed to read response bytes: {}", err),
            });
        }
    };
    let bytes = bytes.to_vec();

    // Create a cursor from the bytes
    let cursor = std::io::Cursor::new(bytes.clone());

    // Create a zip archive from the cursor
    let mut archive = match zip::ZipArchive::new(cursor) {
        Ok(archive) => archive,
        Err(err) => {
            return Err(APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Invalid Zip File".to_string(),
                message: format!("Failed to read zip archive: {}", err),
            });
        }
    };

    // Extract and parse file
    let mut buffer = Vec::new();
    {
        let mut file = match archive.by_name(&file_name) {
            Ok(file) => file,
            Err(_) => {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid Zip File".to_string(),
                    message: format!("Archive does not contain {}", file_name),
                });
            }
        };

        // Read the file contents into a buffer
        if let Err(err) = file.read_to_end(&mut buffer) {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Read Error".to_string(),
                message: format!("Failed to read file contents: {}", err),
            });
        }
    }

    // Create a new cursor and archive for returning
    let return_cursor = std::io::Cursor::new(bytes);
    let return_archive = zip::ZipArchive::new(return_cursor).unwrap();

    Ok(ZipFileContents {
        buffer,
        archive: return_archive,
    })
}
