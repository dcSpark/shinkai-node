#!/bin/bash

export NODE_IP="127.0.0.1"
export NODE_PORT="8082"
export CONNECT_ADDR="127.0.0.1:8080"
export NODE_API_IP="127.0.0.1"
export NODE_API_PORT="3032"
export SECRET_KEY="ILJdRXWXp7BGP5Yg9mbdEKosQ3OlBZZ8fI1wkiotbk4="

if [ -z "$1" ]
then
  echo "No argument supplied for CONNECT_PK, using default"
  export CONNECT_PK="eYy9ZNeMSg+6M4sqY0ljSUDcTltgHbECngLEHg/gVnk="
else
  export CONNECT_PK=$1
fi

cargo run
