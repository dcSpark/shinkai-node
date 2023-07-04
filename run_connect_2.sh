#!/bin/bash

export NODE_IP="127.0.0.1"
export NODE_PORT="8082"
export CONNECT_ADDR="127.0.0.1:8080"
export NODE_API_IP="127.0.0.1"
export NODE_API_PORT="3032"
export IDENTITY_SECRET_KEY="3CdnyVfdgbfd7N5WeWqFGtS6AtkMbT54zUB2dbTzCxj7"
export ENCRYPTION_SECRET_KEY="EUfREpN3gdsNDyXyZx4ZKu64WkjRT5JVzM8MauP4aSKg"
export PING_INTERVAL_SECS="0"

if [ -z "$1" ]
then
  echo "No argument supplied for CONNECT_PK, using default"
  export CONNECT_PK="9BUoYQYq7K38mkk61q8aMH9kD9fKSVL1Fib7FbH6nUkQ"
else
  export CONNECT_PK=$1
fi

cargo run
