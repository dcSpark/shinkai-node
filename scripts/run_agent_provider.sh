#!/bin/bash

export NODE_IP="0.0.0.0"
export NODE_PORT="9752"
export NODE_API_IP="0.0.0.0"
export NODE_API_PORT="9750"
export NODE_API_HTTPS_PORT="9753"
export NODE_WS_PORT="9751"
export IDENTITY_SECRET_KEY="df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119"
export ENCRYPTION_SECRET_KEY="d83f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81159"
export PING_INTERVAL_SECS="0"
export GLOBAL_IDENTITY_NAME="@@agent_provider.sep-shinkai"
export NODE_STORAGE_PATH="agent_provider"
export RUST_LOG=debug,error,info
export STARTING_NUM_QR_PROFILES="1"
export STARTING_NUM_QR_DEVICES="1"
export FIRST_DEVICE_NEEDS_REGISTRATION_CODE="false"
export LOG_SIMPLE="true"
export NO_SECRET_FILE="true"
export EMBEDDINGS_SERVER_URL="http://localhost:11434/"

export INITIAL_AGENT_NAMES="my_gpt"
export INITIAL_AGENT_URLS="https://api.openai.com"
export INITIAL_AGENT_MODELS="openai:gpt-4o-mini"

export CONTRACT_ADDRESS="0x425fb20ba3874e887336aaa7f3fab32d08135ba9"
export ADD_TESTING_NETWORK_ECHO="false"

# Add these lines to enable all log options
export LOG_ALL=1

cargo run
