FROM rust:latest

# Install system dependencies required for GitHub Actions runners and cigen builds.
# We keep this minimal while covering:
#   - Node/npm for JavaScript-based actions (actions/cache, download/upload-artifact)
#   - Protobuf compiler for building the gRPC protocol
#   - Python for scripts and tooling invoked from workflows
#   - git/ca-certificates for network access and repository cloning

RUN set -euxo pipefail \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        gnupg \
        nodejs \
        npm \
        protobuf-compiler \
        python3 \
        python3-pip \
        unzip \
    && rm -rf /var/lib/apt/lists/*

# Enable Corepack (bundled with modern Node) so pnpm/yarn are available without
# additional installs in individual jobs.
RUN corepack enable

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    CARGO_TERM_COLOR=always \
    RUST_BACKTRACE=1

# Pre-install the Rust components we use in CI to avoid repeated downloads.
RUN rustup set profile minimal \
    && rustup component add --toolchain stable clippy rustfmt

WORKDIR /workspace

# Provide a non-root user to match GitHub Actions UID expectations and ensure
# they own the shared Rust toolchain.
RUN useradd -ms /bin/bash runner \
    && chown -R runner:runner "${RUSTUP_HOME}" "${CARGO_HOME}"
USER runner
WORKDIR /workspace
