#!/bin/bash

# sub1 identity_secret_key: 5NMac241NL3bwL1qtGQAPecup4ZHeCW5w47ehESi5gxv 
# identity_public_key: AVjj3u4fL6V3gWqUoP7c3CjYrMH9gMthTpk7JjXKHUTw
# encryption_secret_key: 5NMac241NL3bwL1qtGQAPecup4ZHeCW5w47ehESi5gz2
# encryption_public_key: 9eP1FY8k8BeVThK15vcLZLMV5t2KjbTMG6EqTMTpz7wR

export NODE_IP="127.0.0.1"
export NODE_PORT="8084"
export NODE_API_IP="127.0.0.1"
export NODE_API_PORT="3033"
export IDENTITY_SECRET_KEY="5NMac241NL3bwL1qtGQAPecup4ZHeCW5w47ehESi5gxv"
export ENCRYPTION_SECRET_KEY="5NMac241NL3bwL1qtGQAPecup4ZHeCW5w47ehESi5gz2"
export PING_INTERVAL_SECS="0"

cargo run -- --create_message --receiver_encryption_pk="9BUoYQYq7K38mkk61q8aMH9kD9fKSVL1Fib7FbH6nUkQ" --recipient="@@node1.shinkai" --other="9BUoYQYq7K38mkk61q8aMH9kD9fKSVL1Fib7FbH6nUkQ"
