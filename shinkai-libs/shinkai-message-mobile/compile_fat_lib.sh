#!/bin/bash

# Define variables
PROJECT_NAME="shinkai_message_mobile"
OUTPUT_DIR="./target/universal/release"
IOS_TARGET="aarch64-apple-ios"
IOS_SIM_TARGET="aarch64-apple-ios-sim"
IOS_LIB_DIR="./target/${IOS_TARGET}/release"
IOS_SIM_LIB_DIR="./target/${IOS_SIM_TARGET}/release"
XCFRAMEWORK_DIR="${OUTPUT_DIR}/${PROJECT_NAME}.xcframework"

# Ensure the output directory exists
mkdir -p ${OUTPUT_DIR}

# Build the library for iOS
echo "Building for iOS..."
cargo build --target ${IOS_TARGET} --release

# Build the library for iOS Simulator
echo "Building for iOS Simulator..."
cargo build --target ${IOS_SIM_TARGET} --release

# Create XCFramework
echo "Creating XCFramework..."
xcodebuild -create-xcframework \
    -library ${IOS_LIB_DIR}/lib${PROJECT_NAME}.a \
    -library ${IOS_SIM_LIB_DIR}/lib${PROJECT_NAME}.a \
    -output ${XCFRAMEWORK_DIR}

echo "XCFramework created at ${XCFRAMEWORK_DIR}"