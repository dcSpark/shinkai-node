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
#RUN cargo test -- --test-threads=1


# Create a new stage for the runtime image
#FROM debian:bookworm-slim

# Install any necessary system dependencies
#RUN apt-get update && apt-get install -y \
#    ca-certificates \
#    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from the builder stage to the runtime image
#COPY --from=builder /app/target/debug/shinkai_node /usr/local/bin/shinkai_node

# Set the command to run your application when the container starts
#ENTRYPOINT ["/usr/local/bin/shinkai_node"]

