# Use a Rust base image
FROM rust:bookworm as builder
ARG BUILD_TYPE
RUN apt-get update && apt-get install -y libclang-dev cmake libssl-dev libc++-dev libc++abi-dev lld protobuf-compiler

# Create a new directory for your app
WORKDIR /app

# Copy the Cargo.toml and Cargo.lock files to the container

COPY . .

# Build the dependencies (cached)

RUN cargo clean
RUN rustup component add rustfmt
RUN CARGO_BUILD_RERUN_IF_CHANGED=1 cargo build $([ "$BUILD_TYPE" = "release" ] && echo "--release")

# Runtime stage
FROM debian:bookworm-slim as runner
ARG BUILD_TYPE

# Install runtime dependencies only
RUN apt-get update && apt-get install -y libssl3 ca-certificates

# Copy only necessary files from builder
WORKDIR /app
COPY --from=builder /app/cloud-node/run_node.sh /app/
COPY --from=builder /app/target/${BUILD_TYPE:-debug}/shinkai_node /app/
COPY --from=builder /app/target/${BUILD_TYPE:-debug}/libpdfium.so /app/
COPY --from=builder /app/target/${BUILD_TYPE:-debug}/shinkai-tools-runner-resources /app/shinkai-tools-runner-resources

# Set entrypoint
EXPOSE 9550 9551 9552 9553
ENTRYPOINT ["/bin/sh", "-c", "/app/run_node.sh"]
