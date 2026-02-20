# ============================================================================
# GitWorkflowDashboard — Multi-stage Docker build
# ============================================================================
# No cmake, no libssl-dev needed — reqwest uses rustls (pure Rust TLS)
# Frontend is embedded via include_str! — no Node.js build stage
# ============================================================================

# Stage 1: Build
FROM rust:1.88-bookworm AS builder
WORKDIR /app

# Cache dependency build
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/github src/handlers src/frontend && \
    echo 'fn main() {}' > src/main.rs && \
    echo 'pub mod config; pub mod error; pub mod models;' > src/lib.rs && \
    touch src/config.rs src/error.rs src/models.rs && \
    echo 'pub mod client; pub mod poller; pub mod webhook;' > src/github/mod.rs && \
    touch src/github/client.rs src/github/poller.rs src/github/webhook.rs && \
    echo 'pub mod repos; pub mod runs; pub mod workflows; pub mod ws;' > src/handlers/mod.rs && \
    touch src/handlers/repos.rs src/handlers/workflows.rs src/handlers/runs.rs src/handlers/ws.rs && \
    echo 'pub const INDEX_HTML: &str = "";' > src/frontend/mod.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src

# Build actual application
COPY src/ src/
RUN touch src/main.rs && cargo build --release

# Stage 2: Minimal runtime
FROM debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/git-workflow-dashboard ./

ENV SERVER_HOST=0.0.0.0
ENV SERVER_PORT=4000
ENV RUST_LOG=info

EXPOSE 4000
ENTRYPOINT ["./git-workflow-dashboard"]
