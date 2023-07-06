#!/bin/bash

export NODE_IP="127.0.0.1"
export NODE_PORT="8082"
export CONNECT_ADDR="127.0.0.1:8080"
export NODE_API_IP="127.0.0.1"
export NODE_API_PORT="3032"
export IDENTITY_SECRET_KEY="3c4ErU293cduZWoMhYRveSuTix8yq9gWVg2MemKxXnjP"
export ENCRYPTION_SECRET_KEY="3CdnyVfdgbfd7N5WeWqFGtS6AtkMbT54zUB2dbTzCxj7"
export PING_INTERVAL_SECS="0"

if [ -z "$1" ]
then
  echo "No argument supplied for CONNECT_PK, using default"
  export CONNECT_PK="9d7nvacMcG9kXpSMidcTRkKiAVtmkz8PAjSRXVA7HhwP"
else
  export CONNECT_PK=$1
fi

cargo run
