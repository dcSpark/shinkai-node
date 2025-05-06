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

# Use environment variables if they exist, otherwise use default values
ENV NODE_API_PORT=${NODE_API_PORT:-9550}
ENV NODE_WS_PORT=${NODE_WS_PORT:-9551}
ENV NODE_PORT=${NODE_PORT:-9552}
ENV NODE_HTTPS_PORT=${NODE_HTTPS_PORT:-9553}

EXPOSE ${NODE_API_PORT} ${NODE_WS_PORT} ${NODE_PORT} ${NODE_HTTPS_PORT}

# Set entrypoint
ENTRYPOINT ["/bin/sh", "-c", "/app/run_node.sh"]
