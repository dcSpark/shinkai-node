#!/bin/bash

export NODE_IP="0.0.0.0"
export NODE_PORT="9552"
export NODE_API_IP="0.0.0.0"
export NODE_API_PORT="9550"
export IDENTITY_SECRET_KEY="df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119"
export ENCRYPTION_SECRET_KEY="d83f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81159"
export PING_INTERVAL_SECS="0"
export GLOBAL_IDENTITY_NAME="@@localhost.shinkai"
export RUST_LOG=debug,error,info
export STARTING_NUM_QR_PROFILES="1"
export STARTING_NUM_QR_DEVICES="1"
export FIRST_DEVICE_NEEDS_REGISTRATION_CODE="false"
export LOG_SIMPLE="true"
export NO_SECRET_FILE="true"
export EMBEDDINGS_SERVER_URL="https://public.shinkai.com/x-em"
export UNSTRUCTURED_SERVER_URL="https://public.shinkai.com/x-un"

# export TELEMETRY_ENDPOINT="https://apm-node-b1.shinkai.com/api/default"
# export TELEMETRY_AUTH_HEADER="Basic xxx"

export STATIC_SERVER_PORT="9554"
export STATIC_SERVER_IP="0.0.0.0"
export STATIC_SERVER_FOLDER="./static_server_example"

export INITIAL_AGENT_NAMES="my_gpt,my_gpt_vision,groq"
export INITIAL_AGENT_URLS="https://api.openai.com,https://api.openai.com,https://api.groq.com/openai/v1"
export INITIAL_AGENT_MODELS="openai:gpt-4-1106-preview,openai:gpt-4-vision-preview,groq:llama3-8b-8192"

export RPC_URL="https://rpc.sepolia.org"
export CONTRACT_ADDRESS="0xDCbBd3364a98E2078e8238508255dD4a2015DD3E"
export ABI_PATH="./src/crypto_identities/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json"

# Add these lines to enable all log options
export LOG_ALL=1

cargo run
