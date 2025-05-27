# Multi-stage build for NFS Mamont
FROM rust:1.83.0-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release && rm -rf src

# Copy source code
COPY src ./src
COPY examples ./examples

# Build the actual application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false nfs-user

# Create directories
RUN mkdir -p /app /exports /mnt/nfs && \
    chown -R nfs-user:nfs-user /app /exports

# Copy binary from builder stage
COPY --from=builder /app/target/release/nfs-mamont /app/
COPY --from=builder /app/target/release/examples/* /app/examples/

# Set working directory
WORKDIR /app

# Switch to non-root user
USER nfs-user

# Expose NFS port
EXPOSE 2049

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD timeout 5s nc -z localhost 2049 || exit 1

# Default command
CMD ["./nfs-mamont"] 