use async_channel::{bounded, Receiver, Sender};
use async_std::task;
use reqwest::Identity;
use shinkai_message_wasm::schemas::shinkai_name::{ShinkaiName, ShinkaiSubidentityType};
use shinkai_message_wasm::shinkai_utils::encryption::{
    encryption_public_key_to_string, unsafe_deterministic_encryption_keypair,
};
use shinkai_message_wasm::shinkai_utils::signatures::{
    signature_public_key_to_string, unsafe_deterministic_signature_keypair,
};
use shinkai_message_wasm::shinkai_utils::utils::hash_string;
use shinkai_node::db::db_errors::ShinkaiDBError;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::db::Topic;
use shinkai_node::db::db_identity_registration::RegistrationCodeType;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use shinkai_node::schemas::identity::{IdentityPermissions, IdentityType, StandardIdentity, StandardIdentityType};
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
    let node_profile = ShinkaiName::new(node_profile_name.clone()).unwrap();
    match db.update_local_node_keys(node_profile, encryption_public_key, identity_public_key) {
        Ok(_) => (),
        Err(e) => panic!("Failed to update local node keys: {}", e),
    }
}

#[test]
fn test_generate_and_use_registration_code_for_specific_profile() {
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name.clone()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    // Create a local node profile
    task::block_on(create_local_node_profile(
        &shinkai_db,
        node_profile_name.clone().to_string(),
        encryption_pk.clone(),
        identity_pk.clone(),
    ));

    let profile_name = "profile_1";
    let (profile_identity_sk, profile_identity_pk) = unsafe_deterministic_signature_keypair(1);
    let (profile_encryption_sk, profile_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    // Test generate_registration_code_for_specific_profile
    let registration_code = shinkai_db
        .generate_registration_new_code(IdentityPermissions::Admin, RegistrationCodeType::Profile)
        .unwrap();

    // Test use_registration_code
    shinkai_db
        .use_registration_code(
            &registration_code,
            node_profile_name,
            profile_name,
            &signature_public_key_to_string(profile_identity_pk),
            &encryption_public_key_to_string(profile_encryption_pk),
        )
        .unwrap();

    // check in db
    let profile_name = ShinkaiName::from_node_and_profile(node_profile_name.to_string(), profile_name.to_string()).unwrap();
    let permission_in_db = shinkai_db.get_profile_permission(profile_name).unwrap();
    assert_eq!(permission_in_db, IdentityPermissions::Admin);
}

#[test]
fn test_generate_and_use_registration_code_for_device() {
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name.clone()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    let profile_name = "profile_1";
    let (profile_identity_sk, profile_identity_pk) = unsafe_deterministic_signature_keypair(1);
    let (profile_encryption_sk, profile_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let device_name = "device_1";
    let (device_identity_sk, device_identity_pk) = unsafe_deterministic_signature_keypair(2);
    let (device_encryption_sk, device_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

    // first create the keys for the node
    task::block_on(create_local_node_profile(
        &shinkai_db,
        node_profile_name.clone().to_string(),
        encryption_pk.clone(),
        identity_pk.clone(),
    ));

    // second create a new profile (required to register a device)
    let registration_code = shinkai_db
        .generate_registration_new_code(IdentityPermissions::Admin, RegistrationCodeType::Profile)
        .unwrap();

    let profile_result = shinkai_db
        .use_registration_code(
            &registration_code,
            node_profile_name,
            profile_name,
            &signature_public_key_to_string(profile_identity_pk),
            &encryption_public_key_to_string(profile_encryption_pk),
        );

    println!("profile_result: {:?}", profile_result);
    // registration code for device
    let registration_code = shinkai_db
        .generate_registration_new_code(IdentityPermissions::Standard, RegistrationCodeType::Device(profile_name.to_string()))
        .unwrap();

    // Test use_registration_code
    shinkai_db
        .use_registration_code(
            &registration_code,
            node_profile_name,
            device_name,
            &signature_public_key_to_string(device_identity_pk),
            &encryption_public_key_to_string(device_encryption_pk),
        )
        .unwrap();

    // check in db
    let profile_name = ShinkaiName::from_node_and_profile(node_profile_name.to_string(), profile_name.to_string()).unwrap();
    let permission_in_db = shinkai_db.get_profile_permission(profile_name.clone()).unwrap();
    assert_eq!(permission_in_db, IdentityPermissions::Admin);

    // check device permission
    let device_name = ShinkaiName::from_node_and_profile_and_type_and_name(
        node_profile_name.to_string(),
        profile_name.get_profile_name().unwrap().to_string(),
        ShinkaiSubidentityType::Device,
        device_name.to_string(),
    ).unwrap();
    let permission_in_db = shinkai_db.get_device_permission(device_name).unwrap();
    assert_eq!(permission_in_db, IdentityPermissions::Standard);

}

#[test]
fn test_generate_and_use_registration_code_for_device_with_main_profile() {
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name.clone()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    let device_name = "device_main";
    let (device_identity_sk, device_identity_pk) = unsafe_deterministic_signature_keypair(2);
    let (device_encryption_sk, device_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

    // Create the keys for the node
    task::block_on(create_local_node_profile(
        &shinkai_db,
        node_profile_name.clone().to_string(),
        encryption_pk.clone(),
        identity_pk.clone(),
    ));

    // Generate a registration code for device with main as profile_name
    let registration_code = shinkai_db
        .generate_registration_new_code(IdentityPermissions::Standard, RegistrationCodeType::Device("main".to_string()))
        .unwrap();

    // Use the registration code to create the device and automatically the "main" profile if not exists
    let device_result = shinkai_db
        .use_registration_code(
            &registration_code,
            node_profile_name,
            device_name,
            &signature_public_key_to_string(device_identity_pk),
            &encryption_public_key_to_string(device_encryption_pk),
        );

    // Check if "main" profile exists in db
    let main_profile_name = ShinkaiName::from_node_and_profile(node_profile_name.to_string(), "main".to_string()).unwrap();
    let main_permission_in_db = shinkai_db.get_profile_permission(main_profile_name.clone()).unwrap();
    assert_eq!(main_permission_in_db, IdentityPermissions::Admin);

    // Check if device exists in db
    let device_full_name = ShinkaiName::from_node_and_profile_and_type_and_name(
        node_profile_name.to_string(),
        "main".to_string(),
        ShinkaiSubidentityType::Device,
        device_name.to_string(),
    ).unwrap();
    let device_permission_in_db = shinkai_db.get_device_permission(device_full_name).unwrap();
    assert_eq!(device_permission_in_db, IdentityPermissions::Standard);
}

#[test]
fn test_generate_and_use_registration_code_no_associated_profile() {
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name.clone()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    let profile_name = "profile_1";
    let (profile_identity_sk, profile_identity_pk) = unsafe_deterministic_signature_keypair(1);
    let (profile_encryption_sk, profile_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let device_name = "device_1";
    let (device_identity_sk, device_identity_pk) = unsafe_deterministic_signature_keypair(2);
    let (device_encryption_sk, device_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

    // first create the keys for the node
    task::block_on(create_local_node_profile(
        &shinkai_db,
        node_profile_name.clone().to_string(),
        encryption_pk.clone(),
        identity_pk.clone(),
    ));

    // registration code for device
    let registration_code = shinkai_db
        .generate_registration_new_code(IdentityPermissions::Standard, RegistrationCodeType::Device(profile_name.to_string()))
        .unwrap();

    // Test use_registration_code
    let result = shinkai_db.use_registration_code(
        &registration_code,
        node_profile_name,
        device_name,
        &signature_public_key_to_string(device_identity_pk),
        &encryption_public_key_to_string(device_encryption_pk),
    );

    // Check if an error is returned as no profile is associated
    assert!(
        matches!(result, Err(ShinkaiDBError::ProfileNotFound(node_profile_name))),
        "Expected ProfileNotFound error"
    );
}

#[test]
fn test_new_load_all_sub_identities() {
    setup();
    let node_profile_name = ShinkaiName::new("@@node1.shinkai".to_string()).unwrap();
    let (identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(&node_profile_name.clone().to_string()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    // Create a local node profile
    task::block_on(create_local_node_profile(
        &shinkai_db,
        node_profile_name.clone().to_string(),
        encryption_pk.clone(),
        identity_pk.clone(),
    ));

    // Insert some sub-identities
    for i in 1..=5 {
        let (subidentity_sk, subidentity_pk) = unsafe_deterministic_signature_keypair(i);
        let (subencryption_sk, subencryption_pk) = unsafe_deterministic_encryption_keypair(i);
        let subidentity_name = format!("subidentity_{}", i);

        let identity = StandardIdentity::new(
            ShinkaiName::from_node_and_profile(node_profile_name.to_string(), subidentity_name.to_string()).unwrap(),
            None,
            encryption_pk.clone(),
            identity_pk.clone(),
            Some(subencryption_pk),
            Some(subidentity_pk),
            StandardIdentityType::Profile,
            IdentityPermissions::Standard,
        );

        // check if there was an error and print to console
        shinkai_db
            .insert_profile(identity)
            .unwrap_or_else(|e| println!("Error inserting sub-identity: {}", e));
    }

    // Test new_load_all_sub_identities
    let identities = shinkai_db
        .get_all_profiles(node_profile_name.clone())
        .unwrap_or_else(|e| panic!("Error loading all sub-identities: {}", e));

    assert_eq!(identities.len(), 5);

    // add asserts to check if the identities are correct
    for i in 1..=5 {
        let (subidentity_sk, subidentity_pk) = unsafe_deterministic_signature_keypair(i);
        let (subencryption_sk, subencryption_pk) = unsafe_deterministic_encryption_keypair(i);
        let subidentity_name = format!("subidentity_{}", i);

        let identity = StandardIdentity::new(
            ShinkaiName::from_node_and_profile(node_profile_name.to_string(), subidentity_name.to_string()).unwrap(),
            None,
            encryption_pk.clone(),
            identity_pk.clone(),
            Some(subencryption_pk),
            Some(subidentity_pk),
            // todo: review this
            StandardIdentityType::Profile,
            IdentityPermissions::Standard,
        );

        assert_eq!(identities[(i - 1) as usize], identity);
    }
}

#[test]
fn test_update_local_node_keys() {
    setup();
    let node_profile_name = ShinkaiName::new("@@node1.shinkai".to_string()).unwrap();
    let (identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(&node_profile_name.clone().to_string()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    // Test update_local_node_keys
    shinkai_db
        .update_local_node_keys(
            node_profile_name.clone(),
            encryption_pk.clone(),
            identity_pk.clone(),
        )
        .unwrap();

    // check the encryption and identity keys in database
    let cf_node_encryption = shinkai_db
        .db
        .cf_handle(Topic::ExternalNodeEncryptionKey.as_str())
        .unwrap();
    let cf_node_identity = shinkai_db
        .db
        .cf_handle(Topic::ExternalNodeIdentityKey.as_str())
        .unwrap();
    let encryption_key_in_db = shinkai_db
        .db
        .get_cf(cf_node_encryption, &node_profile_name)
        .unwrap()
        .unwrap();
    let identity_key_in_db = shinkai_db
        .db
        .get_cf(cf_node_identity, &node_profile_name)
        .unwrap()
        .unwrap();

    assert_eq!(
        encryption_key_in_db,
        encryption_public_key_to_string(encryption_pk).as_bytes()
    );
    assert_eq!(
        identity_key_in_db,
        signature_public_key_to_string(identity_pk).as_bytes()
    );
}

#[test]
fn test_new_insert_profile() {
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
        ShinkaiName::from_node_and_profile(node_profile_name.to_string(), subidentity_name.to_string()).unwrap(),
        None,
        encryption_pk.clone(),
        identity_pk.clone(),
        Some(subencryption_pk.clone()),
        Some(subidentity_pk.clone()),
        // todo: review this
        StandardIdentityType::Profile,
        IdentityPermissions::Standard,
    );

    // Test new_insert_sub_identity
    shinkai_db.insert_profile(identity.clone()).unwrap();

    // check in db
    let cf_identity = shinkai_db.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
    let cf_encryption = shinkai_db.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
    let cf_permission = shinkai_db.db.cf_handle(Topic::ProfilesIdentityType.as_str()).unwrap();

    let identity_in_db = shinkai_db
        .db
        .get_cf(cf_identity, identity.full_identity_name.get_profile_name().unwrap())
        .unwrap()
        .unwrap();
    let encryption_in_db = shinkai_db
        .db
        .get_cf(cf_encryption, identity.full_identity_name.get_profile_name().unwrap())
        .unwrap()
        .unwrap();
    let permission_in_db = shinkai_db
        .db
        .get_cf(cf_permission, identity.full_identity_name.get_profile_name().unwrap())
        .unwrap()
        .unwrap();

    assert_eq!(
        identity_in_db,
        signature_public_key_to_string(subidentity_pk).as_bytes()
    );
    assert_eq!(
        encryption_in_db,
        encryption_public_key_to_string(subencryption_pk).as_bytes()
    );
    assert_eq!(permission_in_db, identity.identity_type.to_string().as_bytes());
}

#[test]
fn test_remove_profile() {
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
        ShinkaiName::from_node_and_profile(node_profile_name.to_string(), subidentity_name.to_string()).unwrap(),
        None,
        encryption_pk.clone(),
        identity_pk.clone(),
        Some(subencryption_pk.clone()),
        Some(subidentity_pk.clone()),
        // todo: review this
        StandardIdentityType::Profile,
        IdentityPermissions::Standard,
    );

    // insert identity
    shinkai_db.insert_profile(identity.clone()).unwrap();

    // remove identity
    shinkai_db.remove_profile(&subidentity_name).unwrap();

    // check in db
    let cf_identity = shinkai_db.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
    let cf_encryption = shinkai_db.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
    let cf_permission = shinkai_db.db.cf_handle(Topic::ProfilesIdentityType.as_str()).unwrap();

    let identity_in_db = shinkai_db.db.get_cf(cf_identity, identity.full_identity_name.get_profile_name().unwrap()).unwrap();
    let encryption_in_db = shinkai_db
        .db
        .get_cf(cf_encryption, identity.full_identity_name.get_profile_name().unwrap())
        .unwrap();
    let permission_in_db = shinkai_db
        .db
        .get_cf(cf_permission, identity.full_identity_name.get_profile_name().unwrap())
        .unwrap();

    assert!(identity_in_db.is_none());
    assert!(encryption_in_db.is_none());
    assert!(permission_in_db.is_none());
}
