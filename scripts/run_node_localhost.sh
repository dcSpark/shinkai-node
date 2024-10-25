#!/bin/bash

export NODE_IP="0.0.0.0"
export NODE_PORT="9452"
export NODE_API_IP="0.0.0.0"
export NODE_API_PORT="9450"
export PING_INTERVAL_SECS="0"
export GLOBAL_IDENTITY_NAME="@@localhost.arb-sep-shinkai"
export RUST_LOG=debug,error,info
export STARTING_NUM_QR_PROFILES="1"
export STARTING_NUM_QR_DEVICES="1"
export FIRST_DEVICE_NEEDS_REGISTRATION_CODE="false"
export LOG_SIMPLE="true"
export EMBEDDINGS_SERVER_URL="http://localhost:11434" # assumes that you installed the embeddings server locally using ollama (shinkai-apps helps you handling all of this)

# Add these lines to enable all log options
export LOG_ALL=1

# Check Rust version
rust_required_version="1.76.0"
rust_current_version=$(rustc -V | cut -d ' ' -f 2)

if [ "$(printf '%s\n' "$rust_required_version" "$rust_current_version" | sort -V | head -n1)" = "$rust_current_version" ] && [ "$rust_current_version" != "$rust_required_version" ]; then
    echo "Error: Rust version $rust_required_version or later is required."
    echo "Your current Rust version is $rust_current_version"
    echo "Please update Rust to continue."
    exit 1
fi

echo "Rust version check passed. Current version: $rust_current_version"

# Check for Protobuf Compiler
if ! command -v protoc &> /dev/null; then
    echo "Error: Protobuf Compiler (protoc) is not installed or not in PATH."
    echo "Please install protoc to continue."
    echo "You can download it from https://grpc.io/docs/protoc-installation/"
    exit 1
fi

echo "Protobuf Compiler check passed. $(protoc --version)"

# Check if Ollama is running
if curl -s "$EMBEDDINGS_SERVER_URL" | grep -q "Ollama is running"; then
    echo "Embeddings server found is running and ready."
else
    echo "Error: Ollama is not running or not responding as expected."
    echo "Please make sure Ollama is installed and running on $EMBEDDINGS_SERVER_URL"
    echo ""
    echo "You can download ollama from https://ollama.com/download"
    exit 1
fi

# If all checks pass, proceed with cargo run
cargo run
