#!/bin/bash

export NODE_IP="0.0.0.0"
export NODE_PORT="9552"
export NODE_WS_PORT="9551"
export NODE_API_IP="0.0.0.0"
export NODE_API_PORT="9550"
export IDENTITY_SECRET_KEY="fd1ca428ec1be6ae8b0b3d23ea507eba8cf7da0869578753b9781efda2b6a8ab"
export ENCRYPTION_SECRET_KEY="e06a1c02d638d4552d733dca8ff8f023841d1126965050b2048f1140bfd82a5c"
export PING_INTERVAL_SECS="0"
export GLOBAL_IDENTITY_NAME="@@nico2.shinkai"
export RUST_LOG=debug,error,info
export STARTING_NUM_QR_PROFILES="1"
export STARTING_NUM_QR_DEVICES="1"
export FIRST_DEVICE_NEEDS_REGISTRATION_CODE="false"
export LOG_SIMPLE="true"
export NO_SECRET_FILE="true"
export EMBEDDINGS_SERVER_URL="https://internal.shinkai.com/x-embed-api/embed"
export UNSTRUCTURED_SERVER_URL="https://internal.shinkai.com"

export INITIAL_AGENT_NAMES="my_gpt,my_gpt_vision"
export INITIAL_AGENT_URLS="https://api.openai.com,https://api.openai.com"
export INITIAL_AGENT_MODELS="openai:gpt-4-1106-preview,openai:gpt-4-vision-preview"

# Add these lines to enable all log options
export LOG_ALL=1

cargo run
