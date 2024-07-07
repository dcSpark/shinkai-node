#!/bin/bash

export NODE_IP="0.0.0.0"
export NODE_PORT="9852"
export NODE_API_IP="0.0.0.0"
export NODE_API_PORT="9850"
export NODE_WS_PORT="9851"
export IDENTITY_SECRET_KEY="df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119"
export ENCRYPTION_SECRET_KEY="d83f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81159"
export PING_INTERVAL_SECS="0"
export GLOBAL_IDENTITY_NAME="@@external_identity_testing_tcp_relay.arb-sep-shinkai"
export NODE_STORAGE_PATH="storage_streamer_tcp"
export RUST_LOG=debug,error,info
export STARTING_NUM_QR_PROFILES="1"
export STARTING_NUM_QR_DEVICES="1"
export FIRST_DEVICE_NEEDS_REGISTRATION_CODE="false"
export LOG_SIMPLE="true"
export NO_SECRET_FILE="true"
export EMBEDDINGS_SERVER_URL="http://localhost:11434/"
export UNSTRUCTURED_SERVER_URL="https://public.shinkai.com/x-un"

export INITIAL_AGENT_NAMES="my_gpt,my_gpt_vision"
export INITIAL_AGENT_URLS="https://api.openai.com,https://api.openai.com"
export INITIAL_AGENT_MODELS="openai:gpt-4-1106-preview,openai:gpt-4-vision-preview"

export CONTRACT_ADDRESS="0x1d2D57F78Bc3B878aF68c411a03AcF327c85e0D6"
# Add these lines to enable all log options
export LOG_ALL=1

cargo run
