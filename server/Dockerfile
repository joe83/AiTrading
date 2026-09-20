# ==============================================================================
# Multi-Stage Dockerfile for AI Trading Server (Rust)
# ==============================================================================

# ------------------------------------------------------------------------------
# Stage 1: Build binary
# ------------------------------------------------------------------------------
FROM rust:slim-bookworm AS builder

WORKDIR /usr/src/app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Cache dependency layer: build empty shell with Cargo files first
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Now copy full source code and build production binary
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release --bin ai-trading-server

# ------------------------------------------------------------------------------
# Stage 2: Production runtime
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim AS runner

# Install runtime libraries & curl for healthcheck
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

# Create unprivileged system user
RUN groupadd -g 10001 appgroup && \
    useradd -u 10001 -g appgroup -s /bin/false -m appuser

WORKDIR /app

# Copy binary and migrations from builder
COPY --from=builder /usr/src/app/target/release/ai-trading-server /app/ai-trading-server
COPY --from=builder /usr/src/app/migrations /app/migrations

# Set file ownership
RUN chown -R appuser:appgroup /app

USER appuser:appgroup

# Ports: 8080 = REST API, 8081 = WebSocket
EXPOSE 8080 8081

# Healthcheck against Axum public health endpoint
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:8080/api/health || exit 1

ENTRYPOINT ["/app/ai-trading-server"]
