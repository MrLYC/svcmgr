# syntax=docker/dockerfile:1

# ============================================================
# Stage 1: Build Frontend
# ============================================================
FROM node:20-alpine AS frontend-builder

WORKDIR /app/frontend

# Copy frontend dependencies
COPY src/frontend/package*.json ./

# Install dependencies
RUN npm ci --prefer-offline --no-audit

# Copy frontend source
COPY src/frontend/ ./

# Build frontend
RUN npm run build

# ============================================================
# Stage 2: Build Backend
# ============================================================
FROM rust:1.85-slim-bookworm AS backend-builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libgit2-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source to cache dependencies
RUN mkdir -p src/backend && \
    echo "fn main() {}" > src/backend/main.rs && \
    echo "" > src/backend/lib.rs

# Build dependencies (cached layer)
RUN cargo build --release && \
    rm -rf src/

# Copy actual source code
COPY src/ ./src/
COPY openspec/ ./openspec/

# Build the actual binary
RUN cargo build --release --bin svcmgr

# ============================================================
# Stage 3: Runtime Image
# ============================================================
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libgit2-1.5 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash svcmgr

WORKDIR /home/svcmgr

# Copy binary from builder
COPY --from=backend-builder /app/target/release/svcmgr /usr/local/bin/svcmgr

# Copy frontend build from frontend-builder
COPY --from=frontend-builder /app/frontend/dist /usr/share/svcmgr/frontend

# Set ownership
RUN chown -R svcmgr:svcmgr /home/svcmgr

# Switch to non-root user
USER svcmgr

# Expose default port (if applicable)
EXPOSE 8080

# Set environment variables
ENV RUST_LOG=info
ENV SVCMGR_FRONTEND_PATH=/usr/share/svcmgr/frontend

# Health check (adjust endpoint if needed)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Default command
CMD ["svcmgr", "run"]
