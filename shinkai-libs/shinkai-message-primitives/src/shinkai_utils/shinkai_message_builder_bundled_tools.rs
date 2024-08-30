use crate::{
    schemas::shinkai_proxy_builder_info::ShinkaiProxyBuilderInfo,
    shinkai_utils::encryption::encryption_public_key_to_string,
};
use ed25519_dalek::SigningKey;
use serde::Serialize;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{
    shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::MessageSchemaType},
    shinkai_utils::encryption::EncryptionMethod,
};

use super::shinkai_message_builder::{ShinkaiMessageBuilder, ShinkaiNameString};

impl ShinkaiMessageBuilder {
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn create_generic_invoice_message(
        payload: impl Serialize,
        schema_type: MessageSchemaType,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
        proxy_info: Option<ShinkaiProxyBuilderInfo>,
    ) -> Result<ShinkaiMessage, &'static str> {
        let body = serde_json::to_string(&payload).map_err(|_| "Failed to serialize job creation to JSON")?;

        // It will encrypt the message with the proxy's pk if the sender is localhost and we have a proxy
        let effective_receiver_public_key = if let Some(proxy) = proxy_info {
            if !sender.starts_with("@@localhost.") {
                receiver_public_key
            } else {
                proxy.proxy_enc_public_key
            }
        } else {
            receiver_public_key
        };

        // Convert the encryption secret key to a public key and print it
        let my_encryption_public_key = EncryptionPublicKey::from(&my_encryption_secret_key);
        let my_enc_string = encryption_public_key_to_string(my_encryption_public_key);

        ShinkaiMessageBuilder::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            effective_receiver_public_key,
        )
        .message_raw_content(body)
        .internal_metadata_with_schema(
            sender_subidentity.clone(),
            node_receiver_subidentity.clone(),
            "".to_string(),
            schema_type,
            EncryptionMethod::None,
            None,
        )
        .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
        .external_metadata_with_other_and_intra_sender(node_receiver, sender, my_enc_string, sender_subidentity)
        .build()
    }
}
