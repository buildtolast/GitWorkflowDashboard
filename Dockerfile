# ============================================================================
# GitWorkflowDashboard — Multi-stage Docker build
# ============================================================================
# No cmake, no libssl-dev needed — reqwest uses rustls (pure Rust TLS)
# Build version is injected via GWD_GIT_HASH build arg
# Note: The native Slint UI requires a display server at runtime.
# This Dockerfile is primarily useful for CI builds and testing.
# ============================================================================

# Stage 1: Build
FROM rust:1.93-bookworm AS builder
WORKDIR /app

ARG GWD_GIT_HASH=unknown

# Cache dependency build
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/github src/ui && \
    echo 'fn main() {}' > src/main.rs && \
    echo 'pub mod config; pub mod error; pub mod models; pub mod github; pub mod ui; pub mod repo_store; slint::include_modules!();' > src/lib.rs && \
    touch src/config.rs src/error.rs src/models.rs src/repo_store.rs && \
    echo 'pub mod client; pub mod poller;' > src/github/mod.rs && \
    touch src/github/client.rs src/github/poller.rs && \
    echo 'pub mod actions; pub mod backend; pub mod bridge; pub mod theme;' > src/ui/mod.rs && \
    touch src/ui/actions.rs src/ui/backend.rs src/ui/bridge.rs src/ui/theme.rs && \
    mkdir -p ui && \
    echo 'export component MainWindow inherits Window {}' > ui/main.slint && \
    echo 'fn main() { println!("cargo:rustc-env=GWD_GIT_HASH=cache"); println!("cargo:rustc-env=GWD_BUILT_AT=cache"); slint_build::compile("ui/main.slint").unwrap(); }' > build.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src build.rs ui

# Build actual application
COPY build.rs ./
COPY src/ src/
COPY ui/ ui/
RUN touch src/main.rs && GWD_GIT_HASH=${GWD_GIT_HASH} cargo build --release

# Stage 2: Minimal runtime
FROM debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/git-workflow-dashboard ./

ENV RUST_LOG=info

ENTRYPOINT ["./git-workflow-dashboard"]
