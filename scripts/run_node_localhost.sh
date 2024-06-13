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
# export EMBEDDINGS_SERVER_URL="https://public.shinkai.com/x-em" # if you prefer to use the public embeddings server
export UNSTRUCTURED_SERVER_URL="https://public.shinkai.com/x-un" # we are replacing unstructure soon if you prefer it to install it locally https://docs.shinkai.com/getting-started#unstructured-api

# Add these lines to enable all log options
export LOG_ALL=1

cargo run
