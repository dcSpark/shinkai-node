# Shinkai Network: LibP2P Networking and Identity Guide

This comprehensive guide explains how Shinkai nodes communicate over the network using LibP2P, including identity management, relay configurations, and direct connections.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Shinkai Identity System](#shinkai-identity-system)
3. [Network Modes](#network-modes)
4. [Configuration Guide](#configuration-guide)
5. [Relay Server Setup](#relay-server-setup)
6. [Security and Encryption](#security-and-encryption)
7. [Troubleshooting](#troubleshooting)
8. [Advanced Topics](#advanced-topics)

## Architecture Overview

The Shinkai network uses LibP2P for decentralized peer-to-peer communication with support for both direct connections and relay-based NAT traversal. The system integrates blockchain-based identity verification with modern P2P networking protocols.

### Key Components

- **LibP2P Manager**: Handles peer connections, message routing, and network protocols
- **Identity Registry**: Blockchain-based identity verification on Base Sepolia
- **Relay System**: NAT traversal and peer discovery through relay servers
- **Message Encryption**: End-to-end encryption with forward secrecy

### Network Protocols

- **Identify**: Peer identification and capability exchange
- **Ping**: Connection health monitoring
- **Relay**: Circuit relay for NAT traversal
- **DCUtR**: Direct Connection Upgrade through Relay (hole punching)
- **Request-Response**: Direct message exchange with JSON encoding

## Shinkai Identity System

### Identity Structure

Shinkai identities follow a hierarchical naming convention:
```
@@node.domain/profile/device
```

Examples:
- `@@alice.sep-shinkai` - Global node identity
- `@@alice.sep-shinkai/main` - Profile identity
- `@@alice.sep-shinkai/main/device1` - Device identity

### Identity Components

Each identity consists of:

1. **Signature Keys** (Ed25519): For message signing and identity verification
2. **Encryption Keys** (X25519): For message encryption using Diffie-Hellman
3. **Network Addresses**: LibP2P multiaddresses for connectivity
4. **Routing Information**: Relay servers and connection preferences

### Blockchain Registry

Identities are registered on the Shinkai Registry smart contract:
- **Network**: Base Sepolia testnet
- **Contract**: `0x425fb20ba3874e887336aaa7f3fab32d08135ba9`
- **RPC Endpoints**: Multiple endpoints with automatic failover
- **Management Interface**: https://shinkai-contracts.pages.dev/

#### Identity Registration Requirements

**IMPORTANT**: Each node's Shinkai Identity must be properly configured in the smart contract registry before it can communicate with other nodes. This configuration is essential for peer discovery and connection establishment.

**Required Configuration:**
1. **Public Keys**: Both signature and encryption public keys must be registered
2. **Network Address**: Choose one of the following:
   - **Direct Mode**: Node's public IP address and port
   - **Relay Mode**: Relay server identity (with "use proxy" enabled)

**Registration Process:**
1. Visit https://shinkai-contracts.pages.dev/
2. Connect your wallet (must have Base Sepolia ETH for gas fees)
3. Register your node identity with:
   - Identity name (e.g., `@@my_node.sep-shinkai`)
   - Ed25519 public key (signature key)
   - X25519 public key (encryption key)
   - Network configuration (IP address OR relay identity)

**Direct Mode Registration:**
```
Identity: @@my_node.sep-shinkai
Signature Key: <ed25519_public_key>
Encryption Key: <x25519_public_key>
Network Address: /ip4/203.0.113.42/tcp/9552
Use Proxy: false
```

**Relay Mode Registration:**
```
Identity: @@my_node.sep-shinkai
Signature Key: <ed25519_public_key>
Encryption Key: <x25519_public_key>
Proxy Identity: @@my_relay.sep-shinkai
Use Proxy: true
```

> **Note**: Incorrect or missing registry configuration will prevent other nodes from discovering and connecting to your node. Always verify your registration before attempting to establish connections.

## Network Modes

### Direct Mode

**When to Use:**
- Local development
- Nodes with public IP addresses
- Same network environments
- Testing and debugging

**Characteristics:**
- Direct peer-to-peer connections
- No relay servers required
- Lower latency
- Requires routable IP addresses

**Connection Flow:**
```
Node A ←--→ Node B
```

### Relay Mode

**When to Use:**
- Production deployments
- Nodes behind NAT/firewalls
- Mobile and consumer networks
- Cross-network communication

**Characteristics:**
- NAT traversal through relay servers
- Peer discovery and routing
- Works behind firewalls
- Higher latency but better connectivity

**Connection Flow:**
```
Node A ←--→ Relay Server ←--→ Node B
```

## Configuration Guide

### Quick Setup Checklist

Before configuring your node, ensure you complete these essential steps:

1. **✅ Register Identity**: Visit https://shinkai-contracts.pages.dev/ and register your node identity
2. **✅ Configure Keys**: Set up your Ed25519 and X25519 keys
3. **✅ Choose Network Mode**: Decide between direct or relay mode
4. **✅ Configure Registry**: Set correct network address or proxy identity
5. **✅ Test Connection**: Verify your node can communicate with other nodes

> **⚠️ CRITICAL**: Node identity MUST be registered in the smart contract registry before attempting to connect to other nodes. Without proper registration, peer discovery and connection establishment will fail.

### Environment Variables

#### Core Network Configuration
```bash
# Node Identity
GLOBAL_IDENTITY_NAME="@@node_name.sep-shinkai"

# Network Binding
NODE_IP="0.0.0.0"              # LibP2P listen IP
NODE_PORT="9552"               # LibP2P port
NODE_API_IP="0.0.0.0"          # API server IP
NODE_API_PORT="9550"           # HTTP API port
NODE_WS_PORT="9551"            # WebSocket port
NODE_HTTPS_PORT="9553"         # HTTPS port

# Cryptographic Keys
IDENTITY_SECRET_KEY="32_byte_ed25519_key_in_hex"
ENCRYPTION_SECRET_KEY="32_byte_x25519_key_in_hex"

# Storage
NODE_STORAGE_PATH="./storage"   # Local storage directory
```

#### Mode Selection

**Direct Mode:**
```bash
# Remove or leave empty
export PROXY_IDENTITY=""
# OR
unset PROXY_IDENTITY
```

**Relay Mode:**
```bash
# Set to relay server identity
export PROXY_IDENTITY="@@relay_server.sep-shinkai"
```

### Configuration Examples

#### Local Development (Direct Mode)
```bash
#!/bin/bash
export NODE_API_IP="0.0.0.0"
export NODE_IP="0.0.0.0"
export NODE_API_PORT="9550"
export NODE_WS_PORT="9551"
export NODE_PORT="9552"
export NODE_HTTPS_PORT="9553"
export IDENTITY_SECRET_KEY="df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119"
export ENCRYPTION_SECRET_KEY="d83f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81159"
export GLOBAL_IDENTITY_NAME="@@localhost.sep-shinkai"

# Direct mode - no relay
unset PROXY_IDENTITY

cargo run --bin shinkai_node
```

#### Production Deployment (Relay Mode)
```bash
#!/bin/bash
export NODE_API_IP="0.0.0.0"
export NODE_IP="0.0.0.0"
export NODE_API_PORT="9550"
export NODE_WS_PORT="9551"
export NODE_PORT="9552"
export NODE_HTTPS_PORT="9553"
export IDENTITY_SECRET_KEY="9d662cf50299042d44a4ec7cbe040f96291bbac2d2375515db75bda4046716a9"
export ENCRYPTION_SECRET_KEY="d01d4173c445b6c47000fd4131acbae35a71027d3303c223af013701333bcb54"
export GLOBAL_IDENTITY_NAME="@@production_node.sep-shinkai"

# Relay mode - connect through relay
export PROXY_IDENTITY="@@public_relay.sep-shinkai"

cargo run --bin shinkai_node
```

### Docker Configuration

#### Node Configuration
```dockerfile
ENV NODE_API_IP=0.0.0.0
ENV NODE_IP=0.0.0.0
ENV NODE_API_PORT=9550
ENV NODE_WS_PORT=9551
ENV NODE_PORT=9552
ENV NODE_HTTPS_PORT=9553
ENV IDENTITY_SECRET_KEY=""
ENV ENCRYPTION_SECRET_KEY=""
ENV GLOBAL_IDENTITY_NAME="@@docker_node.sep-shinkai"
ENV PROXY_IDENTITY="@@docker_relay.sep-shinkai"

EXPOSE 9550 9551 9552 9553

CMD ["shinkai_node"]
```

## Relay Server Setup

### Running a Relay Server

#### Command Line
```bash
shinkai_libp2p_relayer \
  --port 9901 \
  --node-name "@@my_relay.sep-shinkai" \
  --identity-secret-key "166a4545bf199c5980777764e8b65f2cb0ed06eec3ed7918a3bf1007aab7c3cc" \
  --encryption-secret-key "f88a6ada6990426a8f8de9f3cec879bc80b4bb4ba4d2441e412b892e40e5a16b" \
  --max-connections 50
```

#### Environment Variables
```bash
export PORT=9901
export IDENTITY_SECRET_KEY="166a4545bf199c5980777764e8b65f2cb0ed06eec3ed7918a3bf1007aab7c3cc"
export ENCRYPTION_SECRET_KEY="f88a6ada6990426a8f8de9f3cec879bc80b4bb4ba4d2441e412b892e40e5a16b"
export GLOBAL_IDENTITY_NAME="@@my_relay.sep-shinkai"
export MAX_CONNECTIONS=50

cd shinkai-libs/shinkai-libp2p-relayer
cargo run
```

#### Docker Deployment
```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin shinkai_libp2p_relayer

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/shinkai_libp2p_relayer /usr/local/bin/

ENV PORT=9901
ENV IDENTITY_SECRET_KEY=""
ENV ENCRYPTION_SECRET_KEY=""
ENV NODE_NAME=""
ENV MAX_CONNECTIONS=20

EXPOSE 9901

CMD ["shinkai_libp2p_relayer"]
```

#### Cloud Deployment (Google Cloud)
```bash
# Deploy with host networking for proper IP detection
docker run -d \
  --name shinkai-relay \
  --network="host" \
  -e PORT=9901 \
  -e NODE_NAME="@@cloud_relay.sep-shinkai" \
  -e IDENTITY_SECRET_KEY="..." \
  -e ENCRYPTION_SECRET_KEY="..." \
  your-relay-image:latest

# Configure firewall
gcloud compute firewall-rules create allow-shinkai-relay \
  --allow tcp:9901,udp:9901 \
  --source-ranges 0.0.0.0/0
```

### Relay Server Features

- **External IP Detection**: Automatically detects public IP for cloud deployments
- **Identity Verification**: Validates connecting nodes against blockchain registry
- **Circuit Management**: Handles relay reservations and connections
- **Message Re-encryption**: Securely forwards messages between peers
- **Node Discovery**: Maintains registry of available nodes and services
- **Connection Limits**: Configurable maximum concurrent connections

## Security and Encryption

### Message Encryption

#### Two-Layer Encryption
1. **Outer Layer**: Node-to-node transport encryption
2. **Inner Layer**: Profile-to-profile application encryption

#### Encryption Process
1. **Key Exchange**: X25519 Diffie-Hellman for session keys
2. **Encryption**: ChaCha20Poly1305 AEAD cipher
3. **Signing**: Ed25519 signatures for authentication
4. **Relay Handling**: Messages re-encrypted at relay servers

#### Security Features
- **Identity Verification**: Blockchain registry validation
- **Message Integrity**: Cryptographic signatures
- **Forward Secrecy**: Ephemeral key generation
- **Replay Protection**: Timestamp and nonce validation

### Key Management

#### Key Generation
```rust
use ed25519_dalek::SigningKey;
use x25519_dalek::{StaticSecret, PublicKey};

// Generate Ed25519 signing key
let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);

// Generate X25519 encryption key
let encryption_key = StaticSecret::random_from_rng(&mut rand::rngs::OsRng);
```

#### Key Storage
- Keys are stored securely in the node's storage directory
- Environment variables for development/testing only
- Production deployments should use secure key management systems

## Troubleshooting

### Common Issues

#### Port Conflicts
```bash
# Error: "API port 9550 is already in use"
# Solution: Change port configuration
export NODE_API_PORT="9560"
export NODE_PORT="9562"
```

#### Invalid Identity Names
```bash
# Error: "Invalid proxy identity name"
# Solution: Use proper identity format
export PROXY_IDENTITY="@@valid_relay.sep-shinkai"
```

#### Relay Connection Failures
```bash
# Enable detailed logging
export RUST_LOG=debug
export LOG_ALL=1

# Check relay server status
curl http://relay_server:9901/health
```

#### Network Connectivity
```bash
# Test port availability
nc -zv 0.0.0.0 9552

# Test relay connectivity
cargo run --example test_connection -- "relay_ip:9901"
```

#### Identity Registration Issues
```bash
# Common issue: "Identity not found in registry"
# Solution: Register identity at https://shinkai-contracts.pages.dev/

# Check if identity is registered
# Visit: https://shinkai-contracts.pages.dev/
# Search for: @@your_node.sep-shinkai

# Verify public keys match your node configuration
# - Ed25519 public key should derive from IDENTITY_SECRET_KEY
# - X25519 public key should derive from ENCRYPTION_SECRET_KEY

# For relay mode, ensure:
# - "Use Proxy" is enabled in registry
# - Proxy Identity field matches your PROXY_IDENTITY env var
# - Relay server is also registered and running

# For direct mode, ensure:
# - Network address matches your public IP and port
# - Firewall allows incoming connections on specified port
```

### Log Analysis

#### Direct Mode Logs
```
🌐 Listening on /ip4/0.0.0.0/tcp/9552 (direct mode)
🔗 LIBP2P Local peer id: 12D3KooW...
📡 Direct connection established with peer: 12D3KooW...
```

#### Relay Mode Logs
```
🌐 Listening on /ip4/0.0.0.0/tcp/9552 (relay mode)
🔗 Setting up LibP2P with relay: @@relay.sep-shinkai
📡 Connecting to relay at: /ip4/x.x.x.x/tcp/9901
🎉 Proxy configured: @@relay.sep-shinkai - using LibP2P relay
```

#### Error Logs
```
❌ Failed to connect to relay: Connection refused
❌ Identity verification failed: Invalid signature
❌ Registry lookup failed: RPC timeout
```

## Advanced Topics

### Custom Relay Configuration

#### Multiple Relay Servers
```bash
# Primary relay
export PROXY_IDENTITY="@@primary_relay.sep-shinkai"

# Fallback configuration (in code)
let fallback_relays = vec![
    "@@backup_relay1.sep-shinkai",
    "@@backup_relay2.sep-shinkai",
];
```

#### Relay Selection Logic
The system automatically selects the best relay based on:
- Connection latency
- Relay server load
- Geographic proximity
- Availability and health status

### Network Optimization

#### Connection Pooling
```rust
// Configure connection limits
let config = NetworkConfig {
    max_connections: 100,
    max_pending_connections: 50,
    connection_timeout: Duration::from_secs(30),
    keep_alive_interval: Duration::from_secs(60),
};
```

#### Message Queuing
```rust
// Configure message queue settings
let queue_config = MessageQueueConfig {
    max_queue_size: 1000,
    retry_attempts: 3,
    retry_delay: Duration::from_secs(5),
    batch_size: 10,
};
```

### Performance Monitoring

#### Metrics Collection
```rust
// Network metrics
let metrics = NetworkMetrics {
    active_connections: peer_count,
    message_throughput: messages_per_second,
    latency: average_latency,
    error_rate: failed_messages / total_messages,
};
```

#### Health Checks
```bash
# Node health endpoint
curl http://localhost:9550/health

# Detailed status
curl http://localhost:9550/v2/status
```

### Development and Testing

#### Local Network Setup
```bash
# Terminal 1: Start relay server
./scripts/run_relay.sh

# Terminal 2: Start first node (relay mode)
export PROXY_IDENTITY="@@local_relay.sep-shinkai"
export GLOBAL_IDENTITY_NAME="@@node1.sep-shinkai"
export NODE_PORT="9552"
./scripts/run_node.sh

# Terminal 3: Start second node (relay mode)
export PROXY_IDENTITY="@@local_relay.sep-shinkai"
export GLOBAL_IDENTITY_NAME="@@node2.sep-shinkai"
export NODE_PORT="9553"
./scripts/run_node.sh
```

#### Integration Testing
```bash
# Run network tests
IS_TESTING=1 cargo test -- --test-threads=1

# Specific libp2p tests
IS_TESTING=1 cargo test libp2p -- --nocapture
```

---

## Conclusion

The Shinkai network provides a robust, secure, and flexible networking layer that supports both direct peer-to-peer connections and relay-based NAT traversal. By combining LibP2P's proven networking protocols with blockchain-based identity verification and strong cryptographic security, Shinkai enables secure communication between nodes regardless of network topology.

This documentation should serve as a comprehensive guide for configuring, deploying, and troubleshooting Shinkai network connections. For additional support or questions, refer to the project's issue tracker or community forums.