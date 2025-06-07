# Shinkai LibP2P Relayer Docker Guide

This guide explains how to build and run the Shinkai LibP2P Relayer using Docker.

## Quick Start

### Prerequisites

1. Docker installed
2. Required environment variables (see Environment Variables section)

### Build and run instructions

1. Build the image from the workspace root:

```bash
# From the shinkai-node root directory
docker build -f shinkai-libs/shinkai-libp2p-relayer/Dockerfile -t shinkai-libp2p-relayer .
```

2. Run the container:

```bash
docker run -d \
  --name shinkai-libp2p-relayer \
  -p 9901:9901 \
  -e IDENTITY_SECRET_KEY="your_identity_secret_key_hex" \
  -e ENCRYPTION_SECRET_KEY="your_encryption_secret_key_hex" \
  -e NODE_NAME="@@your_relay_node.shinkai" \
  -e PORT=9901 \
  -e RPC_URL="https://sepolia.base.org" \
  -e CONTRACT_ADDRESS="0x425fb20ba3874e887336aaa7f3fab32d08135ba9" \
  -e MAX_CONNECTIONS=20 \
  shinkai-libp2p-relayer
```

## Environment Variables

### Required Variables

- `IDENTITY_SECRET_KEY`: Your relay node's identity secret key (hex format)
- `ENCRYPTION_SECRET_KEY`: Your relay node's encryption secret key (hex format)  
- `NODE_NAME`: Your relay node's name (format: `@@your_relay_node.shinkai`)

### Optional Variables

- `PORT`: Port to bind the server (default: 9901)
- `RPC_URL`: RPC URL for registry access (default: https://sepolia.base.org)
- `CONTRACT_ADDRESS`: Registry contract address (default: 0x425fb20ba3874e887336aaa7f3fab32d08135ba9)
- `MAX_CONNECTIONS`: Maximum concurrent connections (default: 20)

## Building Options

### Development Build

```bash
# From the shinkai-node root directory
docker build -f shinkai-libs/shinkai-libp2p-relayer/Dockerfile \
  --build-arg BUILD_TYPE=debug \
  -t shinkai-libp2p-relayer:dev .
```

### Release Build (Default)

```bash
# From the shinkai-node root directory
docker build -f shinkai-libs/shinkai-libp2p-relayer/Dockerfile \
  -t shinkai-libp2p-relayer:latest .
```

## Networking

The container exposes the following:

- **Port 9901**: LibP2P relay server port (configurable via `PORT` env var)

## Security Considerations

1. **Environment Variables**: Store sensitive keys securely, use Docker secrets or external secret management
2. **Network**: Run behind a reverse proxy for production deployments
3. **User**: Container runs as non-root user `shinkai` for security
4. **Registry**: Ensure your relay node is properly registered in the Shinkai registry

## Monitoring

### Health Check

```bash
# Check if container is running
docker ps

# Check container logs for errors
docker logs shinkai-libp2p-relayer | grep -i error
```

### Resource Usage

```bash
# Monitor resource usage
docker stats shinkai-libp2p-relayer
```

## Troubleshooting

### Common Issues

1. **Container won't start**: Check environment variables are set correctly
2. **Connection refused**: Verify port mapping and firewall settings
3. **Registry errors**: Ensure identity keys match registry entries
4. **High memory usage**: Check `MAX_CONNECTIONS` setting

### Accessing Container Shell

```bash
docker run -it --rm \
  --entrypoint /bin/bash \
  shinkai-libp2p-relayer
```

## Production Deployment

### Recommended Production Setup

1. Use Docker secrets for sensitive environment variables
2. Set up proper logging (ELK stack, etc.)
3. Configure reverse proxy (nginx, traefik, etc.)
4. Set up monitoring and alerting
5. Use container orchestration (Docker Swarm, Kubernetes)

## Integration with Other Shinkai Components

This relay can be used with other Shinkai nodes by:

1. Setting the relay's multiaddr in client configurations
2. Ensuring network connectivity between containers
3. Using the same registry configuration across components 