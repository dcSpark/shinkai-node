use async_std::task;
use shinkai_message_primitives::schemas::shinkai_name::{ShinkaiName, ShinkaiSubidentityType};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{IdentityPermissions, RegistrationCodeType};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, unsafe_deterministic_encryption_keypair,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::signatures::{
    signature_public_key_to_string, unsafe_deterministic_signature_keypair,
};
use shinkai_node::db::db_errors::ShinkaiDBError;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::db::Topic;
use shinkai_node::schemas::identity::{StandardIdentity, StandardIdentityType};
use shinkai_vector_resources::utils::hash_string;
use std::fs;
use std::path::Path;

use ed25519_dalek::VerifyingKey;
use x25519_dalek::PublicKey as EncryptionPublicKey;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

async fn create_local_node_profile(
    db: &ShinkaiDB,
    node_profile_name: String,
    encryption_public_key: EncryptionPublicKey,
    identity_public_key: VerifyingKey,
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
    let (_, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (_, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    // Create a local node profile
    task::block_on(create_local_node_profile(
        &shinkai_db,
        node_profile_name.to_string(),
        encryption_pk,
        identity_pk,
    ));

    let profile_name = "profile_1";
    let (_, profile_identity_pk) = unsafe_deterministic_signature_keypair(1);
    let (_, profile_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

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
            None,
            None,
        )
        .unwrap();

    // check in db
    let profile_name =
        ShinkaiName::from_node_and_profile_names(node_profile_name.to_string(), profile_name.to_string()).unwrap();
    let permission_in_db = shinkai_db.get_profile_permission(profile_name).unwrap();
    assert_eq!(permission_in_db, IdentityPermissions::Admin);
}

#[test]
fn test_generate_and_use_registration_code_for_device() {
    
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (_identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (_encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    let profile_name = "profile_1";
    let (_profile_identity_sk, profile_identity_pk) = unsafe_deterministic_signature_keypair(1);
    let (_profile_encryption_sk, profile_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let device_name = "device_1";
    let (_device_identity_sk, device_identity_pk) = unsafe_deterministic_signature_keypair(2);
    let (_device_encryption_sk, device_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

    // first create the keys for the node
    task::block_on(create_local_node_profile(
        &shinkai_db,
        node_profile_name.to_string(),
        encryption_pk,
        identity_pk,
    ));

    // second create a new profile (required to register a device)
    let registration_code = shinkai_db
        .generate_registration_new_code(IdentityPermissions::Admin, RegistrationCodeType::Profile)
        .unwrap();

    let _profile_result = shinkai_db.use_registration_code(
        &registration_code,
        node_profile_name,
        profile_name,
        &signature_public_key_to_string(profile_identity_pk),
        &encryption_public_key_to_string(profile_encryption_pk),
        Some(&signature_public_key_to_string(device_identity_pk)),
        Some(&encryption_public_key_to_string(device_encryption_pk)),
    );

    // registration code for device
    let registration_code = shinkai_db
        .generate_registration_new_code(
            IdentityPermissions::Standard,
            RegistrationCodeType::Device(profile_name.to_string()),
        )
        .unwrap();

    // Test use_registration_code
    shinkai_db
        .use_registration_code(
            &registration_code,
            node_profile_name,
            device_name,
            &signature_public_key_to_string(profile_identity_pk),
            &encryption_public_key_to_string(profile_encryption_pk),
            Some(&signature_public_key_to_string(device_identity_pk)),
            Some(&encryption_public_key_to_string(device_encryption_pk)),
        )
        .unwrap();

    // check in db
    let profile_name =
        ShinkaiName::from_node_and_profile_names(node_profile_name.to_string(), profile_name.to_string()).unwrap();
    let permission_in_db = shinkai_db.get_profile_permission(profile_name.clone()).unwrap();
    assert_eq!(permission_in_db, IdentityPermissions::Admin);

    // check device permission
    let device_name = ShinkaiName::from_node_and_profile_names_and_type_and_name(
        node_profile_name.to_string(),
        profile_name.get_profile_name_string().unwrap().to_string(),
        ShinkaiSubidentityType::Device,
        device_name.to_string(),
    )
    .unwrap();
    let permission_in_db = shinkai_db.get_device_permission(device_name).unwrap();
    assert_eq!(permission_in_db, IdentityPermissions::Standard);
}

#[test]
fn test_generate_and_use_registration_code_for_device_with_main_profile() {
    
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (_identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (_encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    let (_profile_identity_sk, profile_identity_pk) = unsafe_deterministic_signature_keypair(1);
    let (_profile_encryption_sk, profile_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let device_name = "device_main";
    let (_device_identity_sk, device_identity_pk) = unsafe_deterministic_signature_keypair(2);
    let (_device_encryption_sk, device_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

    // Create the keys for the node
    task::block_on(create_local_node_profile(
        &shinkai_db,
        node_profile_name.to_string(),
        encryption_pk,
        identity_pk,
    ));

    // Generate a registration code for device with main as profile_name
    let registration_code = shinkai_db
        .generate_registration_new_code(
            IdentityPermissions::Standard,
            RegistrationCodeType::Device("main".to_string()),
        )
        .unwrap();

    // Use the registration code to create the device and automatically the "main" profile if not exists
    let _device_result = shinkai_db.use_registration_code(
        &registration_code,
        node_profile_name,
        device_name,
        &signature_public_key_to_string(profile_identity_pk),
        &encryption_public_key_to_string(profile_encryption_pk),
        Some(&signature_public_key_to_string(device_identity_pk)),
        Some(&encryption_public_key_to_string(device_encryption_pk)),
    );

    // Check if "main" profile exists in db
    let main_profile_name =
        ShinkaiName::from_node_and_profile_names(node_profile_name.to_string(), "main".to_string()).unwrap();
    let main_permission_in_db = shinkai_db.get_profile_permission(main_profile_name.clone()).unwrap();
    assert_eq!(main_permission_in_db, IdentityPermissions::Admin);

    // Check if device exists in db
    let device_full_name = ShinkaiName::from_node_and_profile_names_and_type_and_name(
        node_profile_name.to_string(),
        "main".to_string(),
        ShinkaiSubidentityType::Device,
        device_name.to_string(),
    )
    .unwrap();
    let device_permission_in_db = shinkai_db.get_device_permission(device_full_name.clone()).unwrap();
    assert_eq!(device_permission_in_db, IdentityPermissions::Standard);

    // Get the device from the database and check that it matches the expected values
    let device_in_db = shinkai_db.get_device(device_full_name.clone()).unwrap();
    assert_eq!(device_in_db.device_signature_public_key, device_identity_pk);
    assert_eq!(device_in_db.device_encryption_public_key, device_encryption_pk);
    assert_eq!(device_in_db.permission_type, IdentityPermissions::Standard);

    assert_ne!(
        device_in_db.profile_encryption_public_key,
        device_in_db.device_encryption_public_key
    );
    assert_ne!(
        device_in_db.profile_signature_public_key,
        device_in_db.device_signature_public_key
    );
}

#[test]
fn test_generate_and_use_registration_code_no_associated_profile() {
    
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (_identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (_encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    let profile_name = "profile_1";
    let (_profile_identity_sk, _profile_identity_pk) = unsafe_deterministic_signature_keypair(1);
    let (_profile_encryption_sk, _profile_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let device_name = "device_1";
    let (_device_identity_sk, device_identity_pk) = unsafe_deterministic_signature_keypair(2);
    let (_device_encryption_sk, device_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

    // first create the keys for the node
    task::block_on(create_local_node_profile(
        &shinkai_db,
        node_profile_name.to_string(),
        encryption_pk,
        identity_pk,
    ));

    // registration code for device
    let registration_code = shinkai_db
        .generate_registration_new_code(
            IdentityPermissions::Standard,
            RegistrationCodeType::Device(profile_name.to_string()),
        )
        .unwrap();

    // Test use_registration_code
    let result = shinkai_db.use_registration_code(
        &registration_code,
        node_profile_name,
        device_name,
        &signature_public_key_to_string(device_identity_pk),
        &encryption_public_key_to_string(device_encryption_pk),
        None,
        None,
    );

    // Check if an error is returned as no profile is associated
    assert!(
        matches!(result, Err(ShinkaiDBError::ProfileNotFound(_node_profile_name))),
        "Expected ProfileNotFound error"
    );
}

#[test]
fn test_new_load_all_sub_identities() {
    
    setup();
    let node_profile_name = ShinkaiName::new("@@node1.shinkai".to_string()).unwrap();
    let (_identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (_encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name.as_ref()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    // Create a local node profile
    task::block_on(create_local_node_profile(
        &shinkai_db,
        node_profile_name.clone().to_string(),
        encryption_pk,
        identity_pk,
    ));

    // Insert some sub-identities
    for i in 1..=5 {
        let (_subidentity_sk, subidentity_pk) = unsafe_deterministic_signature_keypair(i);
        let (_subencryption_sk, subencryption_pk) = unsafe_deterministic_encryption_keypair(i);
        let subidentity_name = format!("subidentity_{}", i);

        let identity = StandardIdentity::new(
            ShinkaiName::from_node_and_profile_names(node_profile_name.to_string(), subidentity_name.to_string())
                .unwrap(),
            None,
            encryption_pk,
            identity_pk,
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
        let (_subidentity_sk, subidentity_pk) = unsafe_deterministic_signature_keypair(i);
        let (_subencryption_sk, subencryption_pk) = unsafe_deterministic_encryption_keypair(i);
        let subidentity_name = format!("subidentity_{}", i);

        let identity = StandardIdentity::new(
            ShinkaiName::from_node_and_profile_names(node_profile_name.to_string(), subidentity_name.to_string())
                .unwrap(),
            None,
            encryption_pk,
            identity_pk,
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
    let (_identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (_encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name.as_ref()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    // Test update_local_node_keys
    shinkai_db
        .update_local_node_keys(node_profile_name.clone(), encryption_pk, identity_pk)
        .unwrap();

    // Update to use the new context for checking the encryption and identity keys in database
    let cf_node_and_users = shinkai_db.db.cf_handle(Topic::NodeAndUsers.as_str()).unwrap();
    let node_name = node_profile_name.get_node_name_string().to_string();
    let encryption_key_prefix = format!("node_encryption_key_{}", node_name);
    let signature_key_prefix = format!("node_signature_key_{}", node_name);

    let encryption_key_in_db = shinkai_db
        .db
        .get_cf(cf_node_and_users, encryption_key_prefix.as_bytes())
        .unwrap()
        .unwrap();
    let identity_key_in_db = shinkai_db
        .db
        .get_cf(cf_node_and_users, signature_key_prefix.as_bytes())
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
    let (_identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (_encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    // Check if any profile exists before insertion
    assert!(
        !shinkai_db.has_any_profile().unwrap(),
        "No profiles should exist before insertion"
    );

    let subidentity_name = "subidentity_1";
    let (_subidentity_sk, subidentity_pk) = unsafe_deterministic_signature_keypair(1);
    let (_subencryption_sk, subencryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let identity = StandardIdentity::new(
        ShinkaiName::from_node_and_profile_names(node_profile_name.to_string(), subidentity_name.to_string()).unwrap(),
        None,
        encryption_pk,
        identity_pk,
        Some(subencryption_pk),
        Some(subidentity_pk),
        StandardIdentityType::Profile,
        IdentityPermissions::Standard,
    );

    // Test insert_profile
    shinkai_db.insert_profile(identity.clone()).unwrap();

    // Check if any profile exists after insertion
    assert!(
        shinkai_db.has_any_profile().unwrap(),
        "A profile should exist after insertion"
    );

    // Update to use the new context for checking in db
    let cf_node_and_users = shinkai_db.db.cf_handle(Topic::NodeAndUsers.as_str()).unwrap();
    let profile_name = identity.full_identity_name.get_profile_name_string().unwrap();
    let identity_key_prefix = format!("identity_key_of_{}", profile_name);
    let encryption_key_prefix = format!("encryption_key_of_{}", profile_name);
    let permission_key_prefix = format!("permissions_of_{}", profile_name);
    let identity_type_key_prefix = format!("identity_type_of_{}", profile_name);

    let identity_in_db = shinkai_db
        .db
        .get_cf(cf_node_and_users, identity_key_prefix.as_bytes())
        .unwrap()
        .unwrap();
    let encryption_in_db = shinkai_db
        .db
        .get_cf(cf_node_and_users, encryption_key_prefix.as_bytes())
        .unwrap()
        .unwrap();
    let permission_in_db = shinkai_db
        .db
        .get_cf(cf_node_and_users, permission_key_prefix.as_bytes())
        .unwrap()
        .unwrap();
    let identity_type_in_db = shinkai_db
        .db
        .get_cf(cf_node_and_users, identity_type_key_prefix.as_bytes())
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
    assert_eq!(permission_in_db, identity.permission_type.to_string().as_bytes());
    assert_eq!(identity_type_in_db, identity.identity_type.to_string().as_bytes());
}

#[test]
fn test_remove_profile() {
    
    setup();
    let node_profile_name = "@@node1.shinkai";
    let (_identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (_encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let db_path = format!("db_tests/{}", hash_string(node_profile_name));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();

    let subidentity_name = "subidentity_1";
    let (_subidentity_sk, subidentity_pk) = unsafe_deterministic_signature_keypair(1);
    let (_subencryption_sk, subencryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let identity = StandardIdentity::new(
        ShinkaiName::from_node_and_profile_names(node_profile_name.to_string(), subidentity_name.to_string()).unwrap(),
        None,
        encryption_pk,
        identity_pk,
        Some(subencryption_pk),
        Some(subidentity_pk),
        StandardIdentityType::Profile,
        IdentityPermissions::Standard,
    );

    // Insert identity
    shinkai_db.insert_profile(identity.clone()).unwrap();

    // Remove identity
    shinkai_db.remove_profile(subidentity_name).unwrap();

    // Update to use the new context for checking in db
    let cf_node_and_users = shinkai_db.db.cf_handle(Topic::NodeAndUsers.as_str()).unwrap();
    let identity_key_prefix = format!("identity_key_of_{}", subidentity_name);
    let encryption_key_prefix = format!("encryption_key_of_{}", subidentity_name);
    let permission_key_prefix = format!("permissions_of_{}", subidentity_name);
    let identity_type_key_prefix = format!("identity_type_of_{}", subidentity_name);

    let identity_in_db = shinkai_db
        .db
        .get_cf(cf_node_and_users, identity_key_prefix.as_bytes())
        .unwrap();
    let encryption_in_db = shinkai_db
        .db
        .get_cf(cf_node_and_users, encryption_key_prefix.as_bytes())
        .unwrap();
    let permission_in_db = shinkai_db
        .db
        .get_cf(cf_node_and_users, permission_key_prefix.as_bytes())
        .unwrap();
    let identity_type_in_db = shinkai_db
        .db
        .get_cf(cf_node_and_users, identity_type_key_prefix.as_bytes())
        .unwrap();

    assert!(identity_in_db.is_none());
    assert!(encryption_in_db.is_none());
    assert!(permission_in_db.is_none());
    assert!(identity_type_in_db.is_none());
}
