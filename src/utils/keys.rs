use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use shinkai_message_primitives::shinkai_utils::{
    encryption::{ephemeral_encryption_keys, string_to_encryption_static_key, clone_static_secret_key, encryption_public_key_to_string, encryption_secret_key_to_string},
    shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
    signatures::{
        clone_signature_secret_key, ephemeral_signature_keypair, signature_secret_key_to_string,
        string_to_signature_secret_key,
    },
};
use std::env;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

pub struct NodeKeys {
    pub identity_secret_key: SignatureStaticKey,
    pub identity_public_key: SignaturePublicKey,
    pub encryption_secret_key: EncryptionStaticKey,
    pub encryption_public_key: EncryptionPublicKey,
}

pub fn generate_or_load_keys() -> NodeKeys {
    let (identity_secret_key, identity_public_key) = match env::var("IDENTITY_SECRET_KEY") {
        Ok(secret_key_str) => {
            let secret_key = string_to_signature_secret_key(&secret_key_str.clone()).unwrap();
            let public_key = SignaturePublicKey::from(&secret_key);

            // Keys Validation (it case of scalar clamp)
            {
                let computed_sk = signature_secret_key_to_string(clone_signature_secret_key(&secret_key));
                if secret_key_str != computed_sk {
                    panic!("Identity secret key is invalid. Original: {} Modified: {}. Recommended to start the node with the modified one from now on.", secret_key_str, computed_sk);
                }
            }

            (secret_key, public_key)
        }
        _ => {
            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Error,
                "No identity secret key found or invalid. Generating ephemeral keys",
            );
            ephemeral_signature_keypair()
        }
    };

    let (encryption_secret_key, encryption_public_key) = match env::var("ENCRYPTION_SECRET_KEY") {
        Ok(secret_key_str) => {
            let secret_key = string_to_encryption_static_key(&secret_key_str).unwrap();
            let public_key = x25519_dalek::PublicKey::from(&secret_key);

            // Keys Validation (it case of scalar clamp)
            {
                let computed_sk = encryption_secret_key_to_string(clone_static_secret_key(&secret_key));
                if secret_key_str != computed_sk {
                    panic!("Encryption secret key is invalid. Original: {} Modified: {}. Recommended to start the node with the modified one from now on.", secret_key_str, computed_sk);
                }
            }

            (secret_key, public_key)
        }
        _ => {
            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Error,
                "No encryption secret key found or invalid. Generating ephemeral keys",
            );
            ephemeral_encryption_keys()
        }
    };

    NodeKeys {
        identity_secret_key,
        identity_public_key,
        encryption_secret_key,
        encryption_public_key,
    }
}
