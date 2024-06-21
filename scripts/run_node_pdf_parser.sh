#!/bin/bash

export NODE_IP="0.0.0.0"
export NODE_PORT="9552"
export NODE_API_IP="0.0.0.0"
export NODE_API_PORT="9550"
export IDENTITY_SECRET_KEY="df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119"
export ENCRYPTION_SECRET_KEY="d83f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81159"
export PING_INTERVAL_SECS="0"
export GLOBAL_IDENTITY_NAME="@@localhost.arb-sep-shinkai"
export RUST_LOG=debug,error,info
export STARTING_NUM_QR_PROFILES="1"
export STARTING_NUM_QR_DEVICES="1"
export FIRST_DEVICE_NEEDS_REGISTRATION_CODE="false"
export LOG_SIMPLE="true"
export NO_SECRET_FILE="true"
export EMBEDDINGS_SERVER_URL="https://internal.shinkai.com/x-embed-api/"
export UNSTRUCTURED_SERVER_URL="https://internal.shinkai.com/x-unstructured-api/"
export NODE_STORAGE_PATH="storage"

export STATIC_SERVER_PORT="9554"
export STATIC_SERVER_IP="0.0.0.0"
export STATIC_SERVER_FOLDER="./static_server_example"

export INITIAL_AGENT_NAMES="llama3_8b"
export INITIAL_AGENT_URLS="http://localhost:11434"
export INITIAL_AGENT_MODELS="ollama:llama3:8b-instruct-q4_1"
export INITIAL_AGENT_API_KEYS=""

# Add these lines to enable all log options
export LOG_ALL=1

# Run this script in the root of the project or adjust the path to the pdfium dynamic library
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    OS="linux"
    ARCH="x64"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    OS="mac"
    if [[ $(uname -m) == "arm64" ]]; then
        ARCH="arm64"
    else
        ARCH="x64"
    fi
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" ]]; then
    OS="win"
    ARCH="x64"
fi

export PDFIUM_DYNAMIC_LIB_PATH=$(PWD)/target/release/pdfium/${OS}-${ARCH}

# Don't use ocrs in debug mode since it is extremely slow
cargo run --release --features dynamic-pdf-parser
