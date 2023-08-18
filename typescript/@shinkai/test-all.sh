#!/bin/bash

cd toolkit-lib \
&& npm ci \
&& npm run test
cd ..

cd toolkit-builder/lib \
&& npm i \
&& npm run compile \
&& npm run test
pwd
cd ..
cd ..

cd toolkit-smtp \
&& npm ci \
&& npm run test
pwd
cd ..

cd toolkit-example \
&& npm ci \
&& npm run test
pwd
cd ..

cd toolkit-google-calendar \
&& npm ci \
&& npm run test
pwd
cd ..

cd toolkit-runner \
&& npm ci \
&& npm run test
pwd
cd ..

cd toolkit-common \
&& npm ci \
&& npm run test
pwd
cd ..
