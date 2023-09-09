# Use a Rust base image
FROM rust:bookworm as builder
RUN apt-get update && apt-get install -y libclang-dev cmake 

# Create a new directory for your app
WORKDIR /app

# Copy the Cargo.toml and Cargo.lock files to the container

COPY . .

# Build the dependencies (cached)

RUN cargo clean 
RUN rustup component add rustfmt
RUN CARGO_BUILD_RERUN_IF_CHANGED=1 cargo build

COPY .github/run-main*.sh /entrypoints/
RUN chmod 755 /entrypoints/*.sh
