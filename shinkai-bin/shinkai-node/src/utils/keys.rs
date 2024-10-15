use ed25519_dalek::{SigningKey, VerifyingKey};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use shinkai_message_primitives::shinkai_utils::{
    encryption::{
        clone_static_secret_key, encryption_secret_key_to_string, ephemeral_encryption_keys,
        string_to_encryption_static_key,
    },
    shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
    signatures::{
        clone_signature_secret_key, ephemeral_signature_keypair, signature_secret_key_to_string,
        string_to_signature_secret_key,
    },
};

use std::{collections::HashMap, env, fs};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

pub struct NodeKeys {
    pub identity_secret_key: SigningKey,
    pub identity_public_key: VerifyingKey,
    pub encryption_secret_key: EncryptionStaticKey,
    pub encryption_public_key: EncryptionPublicKey,
    pub private_https_certificate: Option<String>,
    pub public_https_certificate: Option<String>,
}

pub fn generate_or_load_keys(secrets_file_path: &str) -> NodeKeys {
    // First check for .secret file
    if let Ok(contents) = fs::read_to_string(secrets_file_path) {
        // Parse the contents of the file
        let lines: HashMap<_, _> = contents
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, '=');
                Some((parts.next()?, parts.next()?.to_string()))
            })
            .collect();

        // Use the values from the file if they exist
        if let (Some(identity_secret_key_string), Some(encryption_secret_key_string)) =
            (lines.get("IDENTITY_SECRET_KEY"), lines.get("ENCRYPTION_SECRET_KEY"))
        {
            // Convert the strings back to secret keys
            let identity_secret_key = string_to_signature_secret_key(identity_secret_key_string).unwrap();
            let encryption_secret_key = string_to_encryption_static_key(encryption_secret_key_string).unwrap();

            // Generate public keys from secret keys
            let identity_public_key = identity_secret_key.verifying_key();
            let encryption_public_key = x25519_dalek::PublicKey::from(&encryption_secret_key);

            // Read the HTTPS certificates if they exist
            let private_https_certificate = lines.get("PRIVATE_HTTPS_CERTIFICATE").cloned();
            let public_https_certificate = lines.get("PUBLIC_HTTPS_CERTIFICATE").cloned();

            return NodeKeys {
                identity_secret_key,
                identity_public_key,
                encryption_secret_key,
                encryption_public_key,
                private_https_certificate,
                public_https_certificate,
            };
        }
    }

    // If keys are not found in Stronghold, fall back to ENV or generate new keys
    let (identity_secret_key, identity_public_key) = match env::var("IDENTITY_SECRET_KEY") {
        Ok(secret_key_str) => {
            let secret_key = string_to_signature_secret_key(&secret_key_str.clone()).unwrap();
            let public_key = secret_key.verifying_key();

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

    let (private_https_certificate, public_https_certificate) = match env::var("HTTPS_CERTIFICATE_PEM") {
        Ok(private_key_pem) => {
            // Parse the private key from the PEM string
            let key_pair = KeyPair::from_pem(&private_key_pem).expect("Failed to parse private key");

            // Generate the public certificate from the private key
            let mut params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
            let mut dn = DistinguishedName::new();
            dn.push(DnType::CommonName, "localhost");
            params.distinguished_name = dn;
            let cert = params.self_signed(&key_pair).unwrap();
            let public_cert = cert.pem();

            (private_key_pem, public_cert)
        }
        _ => {
            // Generate a new self-signed certificate
            let mut params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
            let mut dn = DistinguishedName::new();
            dn.push(DnType::CommonName, "localhost");
            params.distinguished_name = dn;
            let key_pair = KeyPair::generate().unwrap();
            let cert = params.self_signed(&key_pair).unwrap();

            // Serialize the private key using KeyPair
            let private_key = key_pair.serialize_pem();

            // Serialize the certificate
            let public_cert = cert.pem();

            (private_key, public_cert)
        }
    };

    NodeKeys {
        identity_secret_key,
        identity_public_key,
        encryption_secret_key,
        encryption_public_key,
        private_https_certificate: Some(private_https_certificate),
        public_https_certificate: Some(public_https_certificate),
    }
}
