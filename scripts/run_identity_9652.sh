#!/bin/bash

export NODE_IP="0.0.0.0"
export NODE_PORT="9652"
export NODE_WS_PORT="9651"
export NODE_API_IP="0.0.0.0"
export NODE_API_PORT="9650"
export NODE_STORAGE_PATH="storage_9652"
export IDENTITY_SECRET_KEY="bf0be9c2da2d5f9371bb61f5b2b9f4a6bb294f064e187056005a8bda8dc2ef00"
export ENCRYPTION_SECRET_KEY="806bbcca2ab460aaa57ed00aa2fdf88b1d039b5ca2d89306d4ebd77b14e52c77"
export PING_INTERVAL_SECS="0"
export GLOBAL_IDENTITY_NAME="@@_my_9652.arb-sep-shinkai"
export RUST_LOG=debug,error,info
export STARTING_NUM_QR_PROFILES="1"
export STARTING_NUM_QR_DEVICES="1"
export FIRST_DEVICE_NEEDS_REGISTRATION_CODE="false"
export LOG_SIMPLE="true"
export NO_SECRET_FILE="true"
export EMBEDDINGS_SERVER_URL="https://internal.shinkai.com/x-embed-api/"
export UNSTRUCTURED_SERVER_URL="https://internal.shinkai.com/x-unstructured-api/"

export INITIAL_AGENT_NAMES="my_gpt,my_gpt_vision"
export INITIAL_AGENT_URLS="https://api.openai.com,https://api.openai.com"
export INITIAL_AGENT_MODELS="openai:gpt-4-1106-preview,openai:gpt-4-vision-preview"

# Add these lines to enable all log options
export LOG_ALL=1

cargo run
