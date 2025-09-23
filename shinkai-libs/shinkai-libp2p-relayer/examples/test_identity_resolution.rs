use shinkai_crypto_identities::ShinkaiRegistry;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing Identity Resolution for LibP2P Relay");
    println!("===============================================");

    let rpc_url = env::var("RPC_URL").unwrap_or("https://sepolia.base.org".to_string());
    let contract_address =
        env::var("CONTRACT_ADDRESS").unwrap_or("0x425fb20ba3874e887336aaa7f3fab32d08135ba9".to_string());

    println!("📡 Using RPC: {}", rpc_url);
    println!("📝 Contract: {}", contract_address);

    let registry = ShinkaiRegistry::new(&rpc_url, &contract_address, None).await?;

    let relay_identity = "@@libp2p_relayer.sep-shinkai";
    println!("\n🔍 Resolving identity: {}", relay_identity);

    match registry
        .get_identity_record(relay_identity.to_string(), Some(true))
        .await
    {
        Ok(identity) => {
            println!("✅ Identity found!");
            println!("   Identity: {}", identity.shinkai_identity);
            println!("   Encryption Key: {}", identity.encryption_key);
            println!("   Signature Key: {}", identity.signature_key);
            println!("   Address(es): {:?}", identity.address_or_proxy_nodes);

            // Try to get the first address
            match identity.first_address().await {
                Ok(addr) => {
                    println!("   📍 Resolved Address: {}", addr);

                    // Check if this matches the expected relay address
                    if addr.to_string() == "34.170.114.216:9901" {
                        println!("✅ Address matches relay external IP!");
                    } else {
                        println!("⚠️  Address does NOT match relay external IP (34.170.114.216:9901)");
                        println!("   Expected: 34.170.114.216:9901");
                        println!("   Found:    {}", addr);
                    }
                }
                Err(e) => {
                    println!("❌ Failed to resolve address: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to resolve identity: {}", e);
            println!("💡 This might be why Shinkai nodes can't connect to the relay");
        }
    }

    // Also test the other identities
    let test_identities = [
        "@@node1_with_libp2p_relayer.sep-shinkai",
        "@@node2_with_libp2p_relayer.sep-shinkai",
    ];

    for identity in &test_identities {
        println!("\n🔍 Testing identity: {}", identity);
        match registry.get_identity_record(identity.to_string(), Some(true)).await {
            Ok(record) => {
                println!("✅ Identity found!");
                println!("   Address(es): {:?}", record.address_or_proxy_nodes);
                match record.first_address().await {
                    Ok(addr) => println!("   📍 Resolved Address: {}", addr),
                    Err(_) => println!("   📍 No address could be resolved"),
                }
            }
            Err(e) => {
                println!("❌ Identity not found: {}", e);
            }
        }
    }

    Ok(())
}
