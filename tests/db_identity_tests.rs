use async_channel::{bounded, Receiver, Sender};
use shinkai_message_wasm::shinkai_utils::encryption::{unsafe_deterministic_encryption_keypair, encryption_public_key_to_string};
use shinkai_message_wasm::shinkai_utils::signatures::{unsafe_deterministic_signature_keypair, signature_public_key_to_string};
use shinkai_message_wasm::shinkai_utils::utils::hash_string;
use shinkai_node::db::Topic;
use async_std::task;
use shinkai_node::db::db_errors::ShinkaiDBError;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::managers::identity_manager::{StandardIdentity, IdentityType};
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

async fn create_local_node_profile(
    db: &ShinkaiDB,
    node_profile_name: String,
    encryption_public_key: EncryptionPublicKey,
    identity_public_key: SignaturePublicKey,
) {
    match db.update_local_node_keys(
        node_profile_name,
        encryption_public_key,
        identity_public_key,
    ) {
        Ok(_) => (),
        Err(e) => panic!("Failed to update local node keys: {}", e),
    }
}

#[test]
fn test_new_load_all_sub_identities() {
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name.clone()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    
    // Create a local node profile
    task::block_on(create_local_node_profile(&shinkai_db, node_profile_name.clone().to_string(), encryption_pk.clone(), identity_pk.clone()));

    // Insert some sub-identities
    for i in 1..=5 {
        let (subidentity_sk, subidentity_pk) = unsafe_deterministic_signature_keypair(i);
        let (subencryption_sk, subencryption_pk) = unsafe_deterministic_encryption_keypair(i);
        let subidentity_name = format!("subidentity_{}", i);

        let identity = StandardIdentity::new(
            subidentity_name.clone(),
            None,
            encryption_pk.clone(),
            identity_pk.clone(),
            Some(subencryption_pk),
            Some(subidentity_pk),
            IdentityType::Device,
        );

        shinkai_db.insert_sub_identity(identity).unwrap();
    }

    // Test new_load_all_sub_identities
    let identities = shinkai_db
        .load_all_sub_identities(node_profile_name.clone().to_string())
        .unwrap();
    assert_eq!(identities.len(), 5);

    // add asserts to check if the identities are correct
    for i in 1..=5 {
        let (subidentity_sk, subidentity_pk) = unsafe_deterministic_signature_keypair(i);
        let (subencryption_sk, subencryption_pk) = unsafe_deterministic_encryption_keypair(i);
        let subidentity_name = format!("subidentity_{}", i);

        let identity = StandardIdentity::new(
            subidentity_name.clone(),
            None,
            encryption_pk.clone(),
            identity_pk.clone(),
            Some(subencryption_pk),
            Some(subidentity_pk),
            IdentityType::Device,
        );

        assert_eq!(identities[(i - 1) as usize], identity);
    }
}

#[test]
fn test_update_local_node_keys() {
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name.clone()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    // Test update_local_node_keys
    shinkai_db.update_local_node_keys(node_profile_name.clone().to_string(), encryption_pk.clone(), identity_pk.clone()).unwrap();


    // check the encryption and identity keys in database
    let cf_node_encryption = shinkai_db.db.cf_handle(Topic::ExternalNodeEncryptionKey.as_str()).unwrap();
    let cf_node_identity = shinkai_db.db.cf_handle(Topic::ExternalNodeIdentityKey.as_str()).unwrap();
    let encryption_key_in_db = shinkai_db.db.get_cf(cf_node_encryption, &node_profile_name).unwrap().unwrap();
    let identity_key_in_db = shinkai_db.db.get_cf(cf_node_identity, &node_profile_name).unwrap().unwrap();

    assert_eq!(encryption_key_in_db, encryption_public_key_to_string(encryption_pk).as_bytes());
    assert_eq!(identity_key_in_db, signature_public_key_to_string(identity_pk).as_bytes());
}

#[test]
fn test_new_insert_sub_identity() {
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name.clone()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    let subidentity_name = "subidentity_1";
    let (subidentity_sk, subidentity_pk) = unsafe_deterministic_signature_keypair(1);
    let (subencryption_sk, subencryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let identity = StandardIdentity::new(
        subidentity_name.to_string(),
        None,
        encryption_pk.clone(),
        identity_pk.clone(),
        Some(subencryption_pk.clone()),
        Some(subidentity_pk.clone()),
        IdentityType::Device,
    );

    // Test new_insert_sub_identity
    shinkai_db.insert_sub_identity(identity.clone()).unwrap();

    // check in db
    let cf_identity = shinkai_db.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
    let cf_encryption = shinkai_db.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
    let cf_permission = shinkai_db.db.cf_handle(Topic::ProfilesIdentityType.as_str()).unwrap();

    let identity_in_db = shinkai_db.db.get_cf(cf_identity, &identity.full_identity_name).unwrap().unwrap();
    let encryption_in_db = shinkai_db.db.get_cf(cf_encryption, &identity.full_identity_name).unwrap().unwrap();
    let permission_in_db = shinkai_db.db.get_cf(cf_permission, &identity.full_identity_name).unwrap().unwrap();

    assert_eq!(identity_in_db, signature_public_key_to_string(subidentity_pk).as_bytes());
    assert_eq!(encryption_in_db, encryption_public_key_to_string(subencryption_pk).as_bytes());
    assert_eq!(permission_in_db, identity.permission_type.to_string().as_bytes());
}

#[test]
fn test_remove_subidentity() {
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name.clone()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    let subidentity_name = "subidentity_1";
    let (subidentity_sk, subidentity_pk) = unsafe_deterministic_signature_keypair(1);
    let (subencryption_sk, subencryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let identity = StandardIdentity::new(
        subidentity_name.to_string(),
        None,
        encryption_pk.clone(),
        identity_pk.clone(),
        Some(subencryption_pk.clone()),
        Some(subidentity_pk.clone()),
        IdentityType::Device,
    );

    // insert identity
    shinkai_db.insert_sub_identity(identity.clone()).unwrap();

    // remove identity
    shinkai_db.remove_subidentity(&subidentity_name).unwrap();

    // check in db
    let cf_identity = shinkai_db.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
    let cf_encryption = shinkai_db.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
    let cf_permission = shinkai_db.db.cf_handle(Topic::ProfilesIdentityType.as_str()).unwrap();

    let identity_in_db = shinkai_db.db.get_cf(cf_identity, &identity.full_identity_name).unwrap();
    let encryption_in_db = shinkai_db.db.get_cf(cf_encryption, &identity.full_identity_name).unwrap();
    let permission_in_db = shinkai_db.db.get_cf(cf_permission, &identity.full_identity_name).unwrap();

    assert!(identity_in_db.is_none());
    assert!(encryption_in_db.is_none());
    assert!(permission_in_db.is_none());
}
