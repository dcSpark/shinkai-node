#!/bin/bash

export NODE_IP="0.0.0.0"
export NODE_PORT="8080"
export NODE_API_IP="0.0.0.0"
export NODE_API_PORT="13013"
export IDENTITY_SECRET_KEY="df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119"
export ENCRYPTION_SECRET_KEY="d83f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81159"
export PING_INTERVAL_SECS="0"
export GLOBAL_IDENTITY_NAME="@@node1.shinkai"
export RUST_LOG=warn,error,info,debug
export STARTING_NUM_QR_PROFILES="1"
export STARTING_NUM_QR_DEVICES="1"

if [ -z "$1" ]
then
  echo "No argument supplied for CONNECT_PK, using empty string"
  export CONNECT_PK=""
else
  export CONNECT_PK=$1
fi

if [ -z "$2" ]
then
  echo "No argument supplied for CONNECT_ADDR, using empty string"
  export CONNECT_ADDR=""
else
  export CONNECT_ADDR=$2
fi

cargo run
