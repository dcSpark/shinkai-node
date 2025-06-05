# Shinkai LibP2P Relayer Usage Guide

## Overview

The Shinkai LibP2P Relayer is a decentralized relay server that enables Shinkai nodes to communicate through peer-to-peer networking using libp2p protocols. This is an alternative to the TCP-based relay system, offering better NAT traversal, peer discovery, and decentralized messaging capabilities.

## Key Differences from TCP Relayer

| Feature | TCP Relayer | LibP2P Relayer |
|---------|-------------|----------------|
| **Architecture** | Centralized TCP server | Decentralized P2P network |
| **NAT Traversal** | Limited, requires port forwarding | Built-in DCUtR support |
| **Peer Discovery** | Manual configuration | Automatic via mDNS and DHT |
| **Message Routing** | Direct TCP connections | Gossipsub pub/sub + direct routing |
| **Scalability** | Limited by server capacity | Distributed load across peers |
| **Reliability** | Single point of failure | Distributed resilience |

## Running the Relay Server

### Prerequisites

1. Set up environment variables:
```bash
export IDENTITY_SECRET_KEY="your_identity_secret_key_hex"
export ENCRYPTION_SECRET_KEY="your_encryption_secret_key_hex"
export NODE_NAME="@@your_relay_node.shinkai"
export RPC_URL="https://sepolia.base.org"
export CONTRACT_ADDRESS="0x425fb20ba3874e887336aaa7f3fab32d08135ba9"
```

2. Ensure your relay node is registered in the Shinkai registry with the corresponding public keys.

### Starting the Server

```bash
# Using cargo run
cd shinkai-libs/shinkai-libp2p-relayer
cargo run -- --port 8080

# Or using the binary directly
cargo build --release
./target/release/shinkai_libp2p_relayer --port 8080
```

### Command Line Options

```bash
shinkai_libp2p_relayer [OPTIONS]

OPTIONS:
    -p, --port <PORT>                    Port to bind the server [default: 8080]
    -i, --identity-key <IDENTITY_KEY>    Identity secret key (hex)
    -e, --encryption-key <ENCRYPTION_KEY> Encryption secret key (hex)
    -n, --node-name <NODE_NAME>          Node name for the relay
    -r, --rpc-url <RPC_URL>              RPC URL for registry access
    -c, --contract-address <CONTRACT>     Registry contract address
    -m, --max-connections <MAX>          Maximum concurrent connections [default: 20]
    -h, --help                           Print help information
    -V, --version                        Print version information
```

## Client Integration

### Connecting Shinkai Nodes

Shinkai nodes can connect to the libp2p relay by:

1. **Automatic Discovery**: If the relay is on the same local network, nodes will discover it via mDNS
2. **Manual Connection**: Connect directly using the relay's multiaddr:
   ```
   /ip4/RELAY_IP/tcp/8080/p2p/RELAY_PEER_ID
   ```

### Message Flow

1. **Registration**: Nodes register their identity with the relay
2. **Topic Subscription**: Nodes subscribe to relevant gossipsub topics
3. **Message Routing**: Messages are routed based on target identity
4. **Direct Communication**: Nodes can establish direct connections when possible

### Example Integration

```rust
use shinkai_libp2p_relayer::{RelayMessage, LibP2PProxy};
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;

// Create a relay message
let message = RelayMessage::new_proxy_message("@@node.shinkai".to_string());

// Send via the relay
relay_proxy.send_message(message).await?;
```

## Network Topology

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Node A    │    │   Node B    │    │   Node C    │
│@@alice.shink│    │@@bob.shinkai│    │@@carol.shink│
└──────┬──────┘    └──────┬──────┘    └──────┬──────┘
       │                  │                  │
       └──────────────────┼──────────────────┘
                          │
                   ┌──────┴──────┐
                   │ LibP2P Relay│
                   │@@relay.shink│
                   └─────────────┘
```

## Topics and Routing

### Gossipsub Topics

- `shinkai-relay-general`: General relay announcements and registrations
- `shinkai-relay-{identity}`: Messages targeted to specific identity
- `shinkai-direct-{peer_id}`: Direct messages to specific peer
- `shinkai-broadcast`: Broadcast messages to all connected peers

### Message Types

1. **ProxyMessage**: Registration and connection management
2. **ShinkaiMessage**: Actual Shinkai protocol messages

## Monitoring and Management

### Health Checks

The relay provides several monitoring endpoints:

```rust
// Get relay statistics
let stats = relay_proxy.get_stats().await;

// List connected peers
let peers = relay_proxy.list_peers().await;
```

### Logs

The relay logs important events:
- Peer connections/disconnections
- Message routing
- Registration events
- Error conditions

## Security Considerations

1. **Identity Validation**: All nodes must be registered in the Shinkai registry
2. **Message Signing**: All gossipsub messages are cryptographically signed
3. **Peer Authentication**: Nodes authenticate via libp2p identify protocol
4. **Rate Limiting**: Built-in connection limits prevent abuse

## Troubleshooting

### Common Issues

1. **Connection Failed**: Check firewall settings and network connectivity
2. **Registration Failed**: Verify identity keys match registry
3. **Message Delivery Failed**: Check topic subscriptions and peer connectivity
4. **High Memory Usage**: Monitor connection count and implement cleanup

### Debug Mode

Enable debug logging:
```bash
RUST_LOG=debug cargo run
```

## Performance Tuning

### Configuration Options

- **Max Connections**: Limit concurrent peer connections
- **Heartbeat Interval**: Adjust gossipsub heartbeat frequency
- **Message Buffer Size**: Configure internal message queues
- **Connection Timeout**: Set peer connection timeouts

### Scaling Considerations

- Deploy multiple relay instances for redundancy
- Use load balancing for high-traffic scenarios
- Monitor resource usage and scale horizontally
- Consider geographic distribution of relays

## Migration from TCP Relayer

1. **Gradual Migration**: Run both relayers in parallel during transition
2. **Client Updates**: Update Shinkai nodes to support libp2p protocols
3. **Configuration Changes**: Update connection strings and discovery methods
4. **Testing**: Thoroughly test message delivery and peer discovery
5. **Monitoring**: Monitor both systems during migration period

## Future Enhancements

- **DHT Integration**: Full Kademlia DHT for global peer discovery
- **Relay Chaining**: Multi-hop relay support for complex topologies
- **Circuit Relay v2**: Enhanced relay protocol support
- **Metrics Export**: Prometheus metrics for monitoring
- **Auto-scaling**: Dynamic relay capacity management 