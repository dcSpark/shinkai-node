#!/bin/bash

export NODE_IP="127.0.0.1"
export NODE_PORT="8081"
export CONNECT_ADDR="127.0.0.1:8080"
export NODE_API_IP="127.0.0.1"
export NODE_API_PORT="3031"
export SECRET_KEY="GGyELi2jbj7K30kZoAgU13jJ445Z+Ua3hEgwOKeXE0s="

if [ -z "$1" ]
then
  echo "No argument supplied for CONNECT_PK, using default"
  export CONNECT_PK="wMD/nPm7n9lfeKZ81+W4jRIYTwDc+EqrzapGi/hAAnw="
else
  export CONNECT_PK=$1
fi

cargo run
