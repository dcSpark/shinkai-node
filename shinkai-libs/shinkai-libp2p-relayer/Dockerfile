# Use a Rust base image for building
FROM rust:bookworm AS builder
ARG BUILD_TYPE=release

# Install build dependencies
RUN apt-get update && apt-get install -y libclang-dev cmake libssl-dev libc++-dev libc++abi-dev lld

# Create a new directory for the app
WORKDIR /app

# Copy the workspace files (build context is workspace root)
COPY Cargo.toml Cargo.lock ./
COPY shinkai-libs ./shinkai-libs
COPY shinkai-bin ./shinkai-bin
COPY shinkai-test-framework ./shinkai-test-framework
COPY shinkai-test-macro ./shinkai-test-macro

# Build the shinkai-libp2p-relayer specifically
RUN rustup component add rustfmt
RUN cargo build --package shinkai_libp2p_relayer --bin shinkai_libp2p_relayer $([ "$BUILD_TYPE" = "release" ] && echo "--release")

# Runtime stage
FROM debian:bookworm-slim AS runner
ARG BUILD_TYPE=release

# Install runtime dependencies including curl for Deno installation
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    curl \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# Install Deno
RUN curl -fsSL https://deno.land/install.sh | sh && \
    mv /root/.deno/bin/deno /usr/local/bin/deno && \
    chmod +x /usr/local/bin/deno

# Create a non-root user for security
RUN useradd -r -s /bin/false -m -d /app shinkai

# Create necessary directories and set permissions for both root and shinkai user
RUN mkdir -p /app/storage /app/internal_tools_storage /app/.cache/deno && \
    chown -R shinkai:shinkai /app && \
    chmod -R 755 /app

# Pre-cache ethers dependency to avoid runtime network issues
RUN echo 'import { ethers } from "npm:ethers@6.14.1"; console.log("Cached ethers");' > /tmp/cache_ethers.ts && \
    /usr/local/bin/deno run --allow-net --allow-read --allow-write --allow-env /tmp/cache_ethers.ts || true && \
    rm /tmp/cache_ethers.ts

# Copy the cache to the shinkai user's directory
RUN cp -r /root/.cache/deno /app/.cache/ && \
    chown -R shinkai:shinkai /app/.cache

# Copy only the necessary binary from builder
WORKDIR /app
COPY --from=builder /app/target/${BUILD_TYPE}/shinkai_libp2p_relayer /app/

# Change ownership to the shinkai user
RUN chown -R shinkai:shinkai /app

# Switch to the non-root user
USER shinkai

# Set environment variables with defaults
ENV PORT=9901
ENV IDENTITY_SECRET_KEY=""
ENV ENCRYPTION_SECRET_KEY=""
ENV NODE_NAME=""
ENV RPC_URL="https://sepolia.base.org"
ENV CONTRACT_ADDRESS="0x425fb20ba3874e887336aaa7f3fab32d08135ba9"
ENV MAX_CONNECTIONS=20
ENV SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH="/usr/local/bin/deno"
ENV NODE_STORAGE_PATH="/app/storage"
ENV DENO_DIR="/app/.cache/deno"

# Expose the port
EXPOSE ${PORT}

# Set entrypoint
ENTRYPOINT ["./shinkai_libp2p_relayer"] 