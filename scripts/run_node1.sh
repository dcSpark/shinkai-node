#!/bin/bash

export NODE_IP="0.0.0.0"
export NODE_PORT="9552"
export NODE_API_IP="0.0.0.0"
export NODE_API_PORT="9550"
export IDENTITY_SECRET_KEY="df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119"
export ENCRYPTION_SECRET_KEY="d83f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81159"
export PING_INTERVAL_SECS="0"
export GLOBAL_IDENTITY_NAME="@@node1.shinkai"
export RUST_LOG=debug,error,info
export STARTING_NUM_QR_PROFILES="1"
export STARTING_NUM_QR_DEVICES="1"
export FIRST_DEVICE_NEEDS_REGISTRATION_CODE="false"

# Add these lines to enable all log options
export LOG_ALL=1

cargo run
