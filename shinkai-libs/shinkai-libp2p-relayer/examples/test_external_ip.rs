use shinkai_libp2p_relayer::relay_manager::RelayManager;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing External IP Detection for LibP2P Relay Manager");
    println!("======================================================");
    
    // Generate a test identity secret key
    let mut rng = OsRng;
    let identity_secret_key = SigningKey::generate(&mut rng);
    
    println!("Creating RelayManager with external IP detection...");
    
    // Create the relay manager - this will automatically detect external IP
    let relay_manager = RelayManager::new(
        9999, // Test port
        "@@test-relay.sep-shinkai".to_string(),
        identity_secret_key,
    ).await?;
    
    // Check if external IP was detected
    if let Some(external_ip) = relay_manager.get_external_ip() {
        println!("✅ External IP successfully detected: {}", external_ip);
        
        // Get external addresses
        let external_addresses = relay_manager.get_external_addresses(9999);
        println!("📍 External addresses for peer connectivity:");
        for addr in external_addresses {
            println!("   - {}", addr);
        }
    } else {
        println!("❌ Failed to detect external IP address");
    }
    
    println!("🔌 Local PeerId: {}", relay_manager.local_peer_id());
    
    Ok(())
} 