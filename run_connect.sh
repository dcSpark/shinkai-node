#!/bin/bash

export NODE_IP="127.0.0.1"
export NODE_PORT="8081"
export CONNECT_ADDR="127.0.0.1:8080"
export NODE_API_IP="127.0.0.1"
export NODE_API_PORT="3031"
export SECRET_KEY="YKvdchAk8P9OCz9ML8E7xbrULQt4UdRW2I0gPRWqpFA="
export PING_INTERVAL="10"

if [ -z "$1" ]
then
  echo "No argument supplied for CONNECT_PK, using default"
  export CONNECT_PK="eYy9ZNeMSg+6M4sqY0ljSUDcTltgHbECngLEHg/gVnk="
else
  export CONNECT_PK=$1
fi

cargo run
