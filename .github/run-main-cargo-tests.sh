#!/bin/bash

export IS_TESTING=1
cd /app && cargo test -- --test-threads=1

