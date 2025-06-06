# Shinkai LibP2P Relay Manager

A LibP2P relay server implementation for the Shinkai network with automatic external IP detection for cloud deployments.

## Features

- 🌐 **Automatic External IP Detection**: Detects public IP addresses for proper external peer connectivity
- 🔄 **Multi-transport Support**: Both TCP and QUIC protocols with fallback support
- 📡 **Gossipsub Messaging**: Efficient peer-to-peer message propagation
- 🔍 **Kademlia DHT**: Distributed peer discovery and routing
- 🛡️ **Relay Protocol**: Allows peers to connect through the relay server
- ☁️ **Cloud-Ready**: Optimized for Google Cloud Platform and other cloud providers

## Google Cloud Deployment

### Problem Solved

When deploying LibP2P relay servers on Google Cloud Platform using Container-Optimized OS with `--network="host"`, the container cannot automatically detect the VM's public IP address. This prevents external peers from connecting to the relay server.

This implementation solves the problem by:

1. **External IP Detection**: Automatically detects the Google Cloud VM's public IP using multiple fallback services
2. **Address Advertisement**: Properly advertises external addresses to the LibP2P network
3. **Cloud Integration**: Seamless operation with Google Cloud's networking model

### Deployment Example

```dockerfile
# Dockerfile for Google Cloud deployment
FROM rust:1.75 as builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin shinkai-libp2p-relayer

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/shinkai-libp2p-relayer /usr/local/bin/

EXPOSE 9090

CMD ["shinkai-libp2p-relayer"]
```

```bash
# Deploy to Google Cloud with host networking
docker run -d \
  --name shinkai-relay \
  --network="host" \
  -e RELAY_PORT=9090 \
  -e NODE_NAME="@@my-relay.sep-shinkai" \
  your-relay-image:latest
```

## Usage

### Basic Setup

```rust
use shinkai_libp2p_relayer::RelayManager;
use ed25519_dalek::SigningKey;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let identity_secret_key = SigningKey::generate(&mut rand::rngs::OsRng);
    
    let mut relay_manager = RelayManager::new(
        9090, // Listen port
        "@@my-relay.sep-shinkai".to_string(),
        identity_secret_key,
    ).await?;
    
    // Check if external IP was detected
    if let Some(external_ip) = relay_manager.get_external_ip() {
        println!("External IP detected: {}", external_ip);
        
        // Get external addresses for advertising
        let addresses = relay_manager.get_external_addresses(9090);
        for addr in addresses {
            println!("External address: {}", addr);
        }
    }
    
    // Start the relay manager
    relay_manager.run().await?;
    
    Ok(())
}
```

### Environment Variables

- `RELAY_PORT`: Port to listen on (default: 9090)
- `NODE_NAME`: Identity name for the relay server
- `IDENTITY_SECRET_KEY`: Ed25519 private key for node identity

## External IP Detection

The relay manager automatically detects external IP addresses using multiple services for reliability:

1. **httpbin.org/ip** - Primary service (JSON response)
2. **api.ipify.org** - Fallback service (plain text)
3. **ifconfig.me/ip** - Secondary fallback
4. **icanhazip.com** - Tertiary fallback

### Detection Process

1. Attempts each service with a 5-second timeout
2. Parses response format (JSON or plain text)
3. Validates IP address format
4. Returns first successful detection
5. Gracefully handles failures and continues without external IP if all services fail

## Network Architecture

```
┌─────────────────────────────────────────────────────────┐
│                Google Cloud VM                          │
│  ┌─────────────────────────────────────────────────────┐│
│  │              Container (--network=host)             ││
│  │  ┌─────────────────────────────────────────────────┐││
│  │  │           LibP2P Relay Manager                  │││
│  │  │                                                 │││
│  │  │  • Binds to 0.0.0.0:9090 (all interfaces)      │││
│  │  │  • Detects external IP: 203.0.113.42           │││
│  │  │  • Advertises: /ip4/203.0.113.42/tcp/9090      │││
│  │  │                /ip4/203.0.113.42/udp/9090/quic │││
│  │  └─────────────────────────────────────────────────┘││
│  └─────────────────────────────────────────────────────┘│
│                                                         │
│  Internal IP: 10.128.0.42                              │
│  External IP: 203.0.113.42                              │
└─────────────────────────────────────────────────────────┘
                           │
                           │ Firewall allows :9090
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│                  Internet Peers                         │
│                                                         │
│  Connect to: /ip4/203.0.113.42/tcp/9090                │
│             /ip4/203.0.113.42/udp/9090/quic-v1         │
└─────────────────────────────────────────────────────────┘
```

## Logging and Monitoring

The relay manager provides detailed logging for monitoring:

```
🌐 External address confirmed and advertised: /ip4/203.0.113.42/tcp/9090
📍 Connection established with peer: 12D3KooW...
⚠️  External address expired: /ip4/203.0.113.42/tcp/9090
```

Key log messages:
- External IP detection attempts and results
- Address advertisement confirmations
- Peer connection events
- Relay reservation acceptances
- Kademlia DHT updates

## Security Considerations

- Uses Ed25519 signatures for peer authentication
- Supports TLS encryption through QUIC transport
- Validates all incoming peer connections
- Implements rate limiting through connection limits

## Troubleshooting

### External IP Detection Fails

If external IP detection fails:

1. Check internet connectivity from the container
2. Verify firewall allows outbound HTTPS (ports 80/443)
3. Check if IP detection services are accessible
4. The relay will still function with local addresses only

### Google Cloud Specific Issues

1. **Firewall Rules**: Ensure the relay port is open:
   ```bash
   gcloud compute firewall-rules create allow-shinkai-relay \
     --allow tcp:9090,udp:9090 \
     --source-ranges 0.0.0.0/0
   ```

2. **Container-Optimized OS**: Use `--network="host"` for direct IP access

3. **External IP Assignment**: Ensure the VM has an external IP assigned

## Dependencies

- `libp2p` 0.55.0+ - Core LibP2P networking
- `reqwest` 0.11+ - HTTP client for IP detection
- `tokio` - Async runtime
- `serde_json` - JSON parsing for IP detection services

## License

This project is licensed under the same terms as the main Shinkai Node project. 