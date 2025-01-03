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
export EMBEDDINGS_SERVER_URL="http://localhost:9081/"
export PROXY_IDENTITY="@@relayer_pub_01.arb-sep-shinkai"
export SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH="${workspaceFolder}/shinkai-bin/shinkai-node/shinkai-tools-runner-resources/deno"

export INITIAL_AGENT_NAMES="o_mixtral"
export INITIAL_AGENT_URLS="http://localhost:11434"
export INITIAL_AGENT_MODELS="ollama:mixtral:8x7b-instruct-v0.1-q4_1"
export INITIAL_AGENT_API_KEYS=""

# Add these lines to enable all log options
export LOG_ALL=1

cargo run
