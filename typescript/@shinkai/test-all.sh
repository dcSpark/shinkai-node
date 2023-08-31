#!/bin/bash

 tookit_root=`pwd`

cd $tookit_root/toolkit-lib \
&& npm ci \
&& npm run test \
&& cd $tookit_root/toolkit-executor \
&& npm ci \
&& npm run test \
&& cd $tookit_root/toolkit-builder/lib \
&& npm i \
&& npm run compile \
&& npm run test \
&& cd $tookit_root/toolkit-smtp \
&& npm ci \
&& npm run compile \
&& npm run test \
&& cd $tookit_root/toolkit-example \
&& npm ci \
&& npm run compile \
&& npm run test \
&& cd $tookit_root/toolkit-google-calendar \
&& npm ci \
&& npm run compile \
&& npm run test \
&& cd $tookit_root/toolkit-web \
&& npm ci \
&& npm run compile \
&& npm run test \
&& cd $tookit_root/toolkit-gmail \
&& npm ci \
&& npm run compile \
&& npm run test
&& echo "All test passed"

