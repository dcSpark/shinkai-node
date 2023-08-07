#!/bin/bash

export NODE_IP="127.0.0.1"
export NODE_PORT="8080"
export NODE_API_IP="127.0.0.1"
export NODE_API_PORT="3030"
export IDENTITY_SECRET_KEY="G2TyLP33XfqndppUzipoTWTs6XnKjmUhCQg1tH44isAG"
export ENCRYPTION_SECRET_KEY="FZ97ouxTGpNnmyyfSBxgC2FGHTpvo7mM7LWoMut6gEYx"
export PING_INTERVAL_SECS="0"

./target/debug/shinkai_node --create_message --receiver_encryption_pk="8NT3CZR16VApT1B5zhinbAdqAvt8QkqMXEiojeFaGdgV" --recipient="@@node2.shinkai"
