# Shinkai LibP2P Relayer

A peer-to-peer relay server built with libp2p that enables Shinkai nodes to communicate through a decentralized relay infrastructure. This relay server provides an alternative to the TCP-based relay system, offering better NAT traversal, peer discovery, and decentralized messaging capabilities.

## Features

- **Decentralized Relay**: Uses libp2p's gossipsub protocol for message broadcasting and routing
- **NAT Traversal**: Built-in support for DCUtR (Direct Connection Upgrade through Relay) protocol
- **Peer Discovery**: Automatic peer discovery through mDNS and identity protocol
- **Message Routing**: Intelligent message routing between registered Shinkai nodes
- **Registry Integration**: Validates node identities against the Shinkai blockchain registry
- **Connection Management**: Efficient connection pooling and rate limiting
- **Monitoring**: Real-time statistics and peer management

## Architecture

The LibP2P Relayer consists of several key components:

### Core Components

1. **RelayManager**: Manages the libp2p swarm and handles network events
2. **LibP2PProxy**: Main relay server that coordinates peer registration and message routing
3. **RelayMessage**: Protocol for wrapping Shinkai messages for relay transmission
4. **RelayBehaviour**: Custom NetworkBehaviour combining gossipsub, identify, ping, and relay protocols

### Protocols Used

- **GossipSub**: For broadcasting and routing messages between peers
- **Identify**: For peer identification and protocol negotiation
- **Ping**: For connection health monitoring
- **Relay**: For NAT traversal and connection relay
- **Noise**: For secure transport encryption
- **Yamux**: For stream multiplexing

## Configuration

The relay server can be configured through environment variables or command-line arguments:

### Required Parameters

- `IDENTITY_SECRET_KEY`: Ed25519 private key for relay identity (hex format)
- `ENCRYPTION_SECRET_KEY`: X25519 private key for message encryption (hex format)
- `NODE_NAME`: Shinkai node name for the relay (e.g., "@@relay.shinkai")

### Optional Parameters

- `PORT`: Listen port for the relay server (default: 8080)
- `RPC_URL`: Blockchain RPC URL for registry validation (default: Sepolia Base)
- `CONTRACT_ADDRESS`: Shinkai registry contract address
- `MAX_CONNECTIONS`: Maximum concurrent connections (default: 20)

## Usage

### Starting the Relay Server

```bash
# Using environment variables
export IDENTITY_SECRET_KEY="your_identity_key_here"
export ENCRYPTION_SECRET_KEY="your_encryption_key_here"
export NODE_NAME="@@relay.shinkai"
export PORT="8080"

cargo run --bin shinkai_libp2p_relayer

# Using command line arguments
cargo run --bin shinkai_libp2p_relayer -- \
  --identity-secret-key "your_identity_key_here" \
  --encryption-secret-key "your_encryption_key_here" \
  --node-name "@@relay.shinkai" \
  --port 8080
```

### Docker Usage

```bash
docker build -t shinkai-libp2p-relayer .
docker run -e IDENTITY_SECRET_KEY="your_key" \
           -e ENCRYPTION_SECRET_KEY="your_key" \
           -e NODE_NAME="@@relay.shinkai" \
           -p 8080:8080 \
           shinkai-libp2p-relayer
```

## Integration with Shinkai Nodes

Shinkai nodes can connect to the libp2p relay by configuring their libp2p manager to use the relay's multiaddr as a relay server:

```rust
// In shinkai-node configuration
let relay_address = "/ip4/relay.example.com/tcp/8080/p2p/12D3KooW...";
let libp2p_manager = LibP2PManager::new(
    node_name,
    listen_port,
    message_handler,
    Some(relay_address.parse().unwrap()),
).await?;
```

## Message Flow

1. **Registration**: Shinkai nodes connect to the relay and register their identity
2. **Authentication**: The relay validates node identities against the blockchain registry  
3. **Message Routing**: Messages are routed through gossipsub topics based on target identity
4. **Delivery**: Messages are delivered to target peers through the relay network

## Security

- **Identity Validation**: All connecting nodes must have valid registry entries
- **Encrypted Transport**: All communications use Noise protocol encryption
- **Rate Limiting**: Built-in connection and message rate limiting
- **Signature Verification**: Messages are cryptographically signed and verified

## Monitoring

The relay provides several monitoring capabilities:

- Connection statistics (active peers, connection limits)
- Message routing metrics
- Peer registry and health status
- LibP2P swarm diagnostics

## Differences from TCP Relayer

| Feature | TCP Relayer | LibP2P Relayer |
|---------|-------------|----------------|
| Protocol | TCP | LibP2P (multiple protocols) |
| NAT Traversal | Limited | Built-in DCUtR support |
| Peer Discovery | Manual | Automatic (mDNS, DHT) |
| Message Routing | Direct TCP | GossipSub pub/sub |
| Scalability | Connection-based | Topic-based |
| Resilience | Single point of failure | Distributed relay network |

## Development

### Building

```bash
cd shinkai-libs/shinkai-libp2p-relayer
cargo build --release
```

### Testing

```bash
cargo test
```

### Dependencies

The relayer depends on:
- `libp2p` 0.53 with gossipsub, relay, and other protocols
- `shinkai-message-primitives` for message handling
- `shinkai-crypto-identities` for registry integration
- Standard async runtime (`tokio`)

## Future Enhancements

- **Multi-relay Networks**: Support for connecting multiple relay servers
- **Advanced Routing**: Content-based routing and message filtering
- **Metrics Export**: Prometheus metrics for monitoring
- **Load Balancing**: Intelligent load distribution across relay nodes
- **Circuit Relay v2**: Upgrade to latest libp2p relay protocol
- **WebRTC Support**: Browser-based peer connections

## License

This project is licensed under the same terms as the main Shinkai project. 