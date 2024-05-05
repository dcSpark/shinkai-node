#!/bin/bash

export IS_TESTING=1
export WELCOME_MESSAGE=false
cd /app && cargo test -- --test-threads=1
