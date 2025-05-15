#!/bin/bash

export INSTALL_FOLDER_PATH=${INSTALL_FOLDER_PATH:-"/app/pre-install"}
cd /app/shinkai-libs/shinkai-message-primitives && cargo test -- --test-threads=1 --nocapture

