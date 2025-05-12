#!/bin/bash

export IS_TESTING=1
export WELCOME_MESSAGE=false
export INSTALL_FOLDER_PATH=${INSTALL_FOLDER_PATH:-"/app/pre-install"}
cd /app && cargo test -- --test-threads=1
