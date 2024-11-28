#!/bin/bash -eu

TIMESTAMP=$(date +%y%m%d%H%M%S)
CPU=arm64
LINKING=dynamic

docker build --platform linux/amd64 -t build-pdfium-$TIMESTAMP -f Dockerfile .

docker run --platform linux/amd64 -v $(PWD)/linux-$CPU:/app/linux-$CPU -e LINKING=$LINKING -e CPU=$CPU --name build-pdfium-$TIMESTAMP build-pdfium-$TIMESTAMP
