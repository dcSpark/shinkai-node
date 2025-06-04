use shinkai_libp2p_relayer::RelayMessage;
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;

#[tokio::test]
async fn test_relay_message_serialization() {
    // Test basic message serialization
    let message = RelayMessage::new_proxy_message("@@test.shinkai".to_string());
    
    // Serialize and deserialize
    let bytes = message.to_bytes().expect("Should serialize");
    let deserialized = RelayMessage::from_bytes(&bytes).expect("Should deserialize");
    
    assert_eq!(message.identity, deserialized.identity);
    assert_eq!(message.message_type, deserialized.message_type);
    assert_eq!(message.payload, deserialized.payload);
    assert_eq!(message.target_peer, deserialized.target_peer);
}

#[tokio::test]
async fn test_proxy_message_creation() {
    let identity = "@@test.shinkai".to_string();
    let message = RelayMessage::new_proxy_message(identity.clone());
    
    assert_eq!(message.identity, identity);
    assert_eq!(message.message_type, NetworkMessageType::ProxyMessage);
    assert!(message.payload.is_empty());
    assert!(message.target_peer.is_none());
}
