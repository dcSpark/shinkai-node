#!/bin/bash

export NODE_IP="127.0.0.1"
export NODE_PORT="8081"
export NODE_API_IP="0.0.0.0"
export NODE_API_PORT="8089"
export IDENTITY_SECRET_KEY="67abdd721024f0ff4e0b3f4c2fc13bc5bad42d0b7851d456d88d203d15aaa450"
export ENCRYPTION_SECRET_KEY="60abdd721024f0ff4e0b3f4c2fc13bc5bad42d0b7851d456d88d203d15aaa450"
export PING_INTERVAL_SECS="0"
export GLOBAL_IDENTITY_NAME="@@node2.shinkai"
export RUST_LOG=warn,error,info,debug
export EMBEDDINGS_SERVER_URL="https://internal.shinkai.com/x-embed-api/embed"
export UNSTRUCTURED_SERVER_URL="https://internal.shinkai.com"

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
