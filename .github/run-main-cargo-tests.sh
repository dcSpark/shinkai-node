#!/bin/bash

export IS_TESTING=1
export SKIP_IMPORT_FROM_DIRECTORY=true
export WELCOME_MESSAGE=false
cd /app && cargo test -- --test-threads=1
