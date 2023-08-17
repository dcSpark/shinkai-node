#!/bin/bash

cd toolkit-builder/lib \
&& npm run compile \
&& npm i \
&& npm run test
echo pwd
cd ..
cd ..

cd toolkit-smtp \
&& npm ci \
&& npm run test
echo pwd
cd ..

cd toolkit-example \
&& npm ci \
&& npm run test
echo pwd
cd ..

cd toolkit-google-calendar \
&& npm ci \
&& npm run test
echo pwd
cd ..

cd toolkit-runner \
&& npm ci \
&& npm run test
echo pwd
cd ..

cd toolkit-common \
&& npm ci \
&& npm run test
echo pwd
cd ..
