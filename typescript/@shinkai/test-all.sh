#!/bin/bash

cd toolkit-builder/lib \
&& npm run compile \
&& npm i \
&& npm run test
cd ..
cd ..

cd toolkit-example \
&& npm ci \
&& npm run test
cd ..

cd toolkit-google-calendar \
&& npm ci \
&& npm run test
cd ..

cd toolkit-runner \
&& npm ci \
&& npm run test
cd ..

