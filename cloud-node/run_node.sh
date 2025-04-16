#!/bin/bash

export NODE_API_IP=0.0.0.0
export NODE_IP=0.0.0.0
export NODE_API_PORT=9550
export NODE_WS_PORT=9551
export NODE_PORT=9552
export NODE_HTTPS_PORT=9553
export SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH="/app/shinkai-tools-runner-resources/deno"
export SHINKAI_TOOLS_RUNNER_UV_BINARY_PATH="/app/shinkai-tools-runner-resources/uv"
export PATH="/app/shinkai-tools-runner-resources:/root/.local/bin:$PATH"

/app/shinkai_node