#!/bin/bash

NVM_DIR=/usr/local/nvm
NODE_VERSION=v16.20.1

PATH=$PATH:$NVM_DIR/versions/node/$NODE_VERSION/bin:$PATH
RUN node -v 


cd /app/shinkai-app
source $NVM_DIR/nvm.sh && nvm use
npm run test.unit
