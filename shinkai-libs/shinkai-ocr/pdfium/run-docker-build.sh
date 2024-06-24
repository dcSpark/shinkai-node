#!/bin/bash -eu

TIMESTAMP=$(date +%y%m%d%H%M%S)

docker build -t build-pdfium-$TIMESTAMP -f Dockerfile .

docker run -v $(PWD)/linux-x64:/app/linux-x64 --name build-pdfium-$TIMESTAMP build-pdfium-$TIMESTAMP
