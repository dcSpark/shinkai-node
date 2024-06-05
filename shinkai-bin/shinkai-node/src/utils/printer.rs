use shinkai_message_primitives::shinkai_utils::{signatures::{unsafe_deterministic_signature_keypair, signature_secret_key_to_string, signature_public_key_to_string}, encryption::{unsafe_deterministic_encryption_keypair, encryption_secret_key_to_string, encryption_public_key_to_string}};


fn print_generated_keys() {
    let _node1_identity_name = "@@node1.shinkai";
    let _node2_identity_name = "@@node2.shinkai";

    let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
    let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let (node3_identity_sk, node3_identity_pk) = unsafe_deterministic_signature_keypair(2);
    let (node3_encryption_sk, node3_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

    let (sub1_identity_sk, sub1_identity_pk) = unsafe_deterministic_signature_keypair(100);
    let (sub1_encryption_sk, sub1_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

    println!(
        "node1 identity_secret_key: {} identity_public_key: {} encryption_secret_key: {} encryption_public_key: {}",
        signature_secret_key_to_string(node1_identity_sk),
        signature_public_key_to_string(node1_identity_pk),
        encryption_secret_key_to_string(node1_encryption_sk),
        encryption_public_key_to_string(node1_encryption_pk)
    );

    println!("node2 identity_secret_key: {} identity_public_key: {} encryption_secret_key: {} encryption_public_key: {}", signature_secret_key_to_string(node2_identity_sk), signature_public_key_to_string(node2_identity_pk), encryption_secret_key_to_string(node2_encryption_sk), encryption_public_key_to_string(node2_encryption_pk));
    println!("node3 identity_secret_key: {} identity_public_key: {} encryption_secret_key: {} encryption_public_key: {}", signature_secret_key_to_string(node3_identity_sk), signature_public_key_to_string(node3_identity_pk), encryption_secret_key_to_string(node3_encryption_sk), encryption_public_key_to_string(node3_encryption_pk));
    println!(
        "sub1 identity_secret_key: {} identity_public_key: {} encryption_secret_key: {} encryption_public_key: {}",
        signature_secret_key_to_string(sub1_identity_sk),
        signature_public_key_to_string(sub1_identity_pk),
        encryption_secret_key_to_string(sub1_encryption_sk),
        encryption_public_key_to_string(sub1_encryption_pk)
    );
}
