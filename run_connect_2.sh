#!/bin/bash

export NODE_IP="127.0.0.1"
export NODE_PORT="8082"
export CONNECT_ADDR="127.0.0.1:8080"
export NODE_API_IP="127.0.0.1"
export NODE_API_PORT="3032"
export SECRET_KEY="UEbkn/SV8f1DaBRs9gw44rFkGRFYwGn5fHHSeg0vVFY="

if [ -z "$1" ]
then
  echo "No argument supplied for CONNECT_PK, using default"
  export CONNECT_PK="wMD/nPm7n9lfeKZ81+W4jRIYTwDc+EqrzapGi/hAAnw="
else
  export CONNECT_PK=$1
fi

cargo run
