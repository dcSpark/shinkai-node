#!/bin/bash

export IS_TESTING=1
export WELCOME_MESSAGE=false
export PDFIUM_DYNAMIC_LIB_PATH=/app/shinkai-bin/shinkai-executor/pdfium/linux-x64/libpdfium.so
cd /app && cargo test -- --test-threads=1
