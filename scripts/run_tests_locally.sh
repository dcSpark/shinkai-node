#!/bin/bash

# Update the tools
./scripts/update_tools.sh

# Get the short sha of the current commit
SHORT_SHA=$(git rev-parse --short HEAD)

# Build the testing image
docker build -t testing_image:${SHORT_SHA} -f .github/Dockerfile .

# Run primitives cargo tests
docker run --rm --entrypoint /entrypoints/run-main-primitives-cargo-tests.sh testing_image:${SHORT_SHA} || true

# Run main cargo tests
docker run --rm --entrypoint /entrypoints/run-main-cargo-tests.sh testing_image:${SHORT_SHA} || true

# Remove the testing image
docker rmi testing_image:${SHORT_SHA} || true
