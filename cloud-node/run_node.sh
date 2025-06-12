#!/bin/bash

# Set default values
DEFAULT_NODE_API_IP=0.0.0.0
DEFAULT_NODE_IP=0.0.0.0
DEFAULT_NODE_API_PORT=9550
DEFAULT_NODE_WS_PORT=9551
DEFAULT_NODE_PORT=9552
DEFAULT_NODE_HTTPS_PORT=9553
DEFAULT_INSTALL_FOLDER_PATH="/app/pre-install"

# Use environment variables if defined, otherwise use defaults
export NODE_API_IP=${NODE_API_IP:-$DEFAULT_NODE_API_IP}
export NODE_IP=${NODE_IP:-$DEFAULT_NODE_IP}
export NODE_API_PORT=${NODE_API_PORT:-$DEFAULT_NODE_API_PORT}
export NODE_WS_PORT=${NODE_WS_PORT:-$DEFAULT_NODE_WS_PORT}
export NODE_PORT=${NODE_PORT:-$DEFAULT_NODE_PORT}
export NODE_HTTPS_PORT=${NODE_HTTPS_PORT:-$DEFAULT_NODE_HTTPS_PORT}
export INSTALL_FOLDER_PATH=${INSTALL_FOLDER_PATH:-$DEFAULT_INSTALL_FOLDER_PATH}
export SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH="/app/shinkai-tools-runner-resources/deno"
export SHINKAI_TOOLS_RUNNER_UV_BINARY_PATH="/app/shinkai-tools-runner-resources/uv"
export PATH="/app/shinkai-tools-runner-resources:/root/.local/bin:$PATH"

echo "Shinkai node port definitions:"
echo "NODE_API_IP: $NODE_API_IP"
echo "NODE_IP: $NODE_IP"
echo "NODE_API_PORT: $NODE_API_PORT"
echo "NODE_WS_PORT: $NODE_WS_PORT"
echo "NODE_PORT: $NODE_PORT"
echo "NODE_HTTPS_PORT: $NODE_HTTPS_PORT"

/app/shinkai_node