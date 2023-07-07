#!/bin/bash

export NODE_IP="127.0.0.1"
export NODE_PORT="8081"
export CONNECT_ADDR="127.0.0.1:8080"
export NODE_API_IP="127.0.0.1"
export NODE_API_PORT="3031"
export IDENTITY_SECRET_KEY="7ygzVHMWYi3DRRZgADgUch6mvJS5YtYa1Mob8kkLuhdR"
export ENCRYPTION_SECRET_KEY="7WN8xpGvHraDZairbgpMMCtB7EUgcEqDvHeNcPaNs511"
export PING_INTERVAL_SECS="0"
export GLOBAL_IDENTITY_NAME="@@node2.shinkai"

if [ -z "$1" ]
then
  echo "No argument supplied for CONNECT_PK, using default"
  export CONNECT_PK="9d7nvacMcG9kXpSMidcTRkKiAVtmkz8PAjSRXVA7HhwP"
else
  export CONNECT_PK=$1
fi

cargo run
