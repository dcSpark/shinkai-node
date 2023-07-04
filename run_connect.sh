#!/bin/bash

export NODE_IP="127.0.0.1"
export NODE_PORT="8081"
export CONNECT_ADDR="127.0.0.1:8080"
export NODE_API_IP="127.0.0.1"
export NODE_API_PORT="3031"
export SECRET_KEY="7WN8xpGvHraDZairbgpMMCtB7EUgcEqDvHeNcPaNs511"
export PING_INTERVAL_SECS="0"

if [ -z "$1" ]
then
  echo "No argument supplied for CONNECT_PK, using default"
  export CONNECT_PK="9BUoYQYq7K38mkk61q8aMH9kD9fKSVL1Fib7FbH6nUkQ"
else
  export CONNECT_PK=$1
fi

cargo run
