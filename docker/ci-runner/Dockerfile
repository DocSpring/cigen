FROM rust:latest

ARG CIGEN_VERSION=0.1.0

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

ENV CIGEN_PLUGIN_DIR=/usr/local/lib/cigen/plugins

RUN set -euxo pipefail; \
    arch="$(uname -m)"; \
    case "$arch" in \
      x86_64) asset_suffix="linux_amd64" ;; \
      aarch64) asset_suffix="linux_arm64" ;; \
      *) echo "Unsupported architecture: $arch" >&2; exit 1 ;; \
    esac; \
    tarball="cigen_${CIGEN_VERSION}_${asset_suffix}.tar.gz"; \
    curl -fsSL -o "/tmp/${tarball}" "https://github.com/DocSpring/cigen/releases/download/v${CIGEN_VERSION}/${tarball}"; \
    curl -fsSL -o "/tmp/${tarball}.sha256" "https://github.com/DocSpring/cigen/releases/download/v${CIGEN_VERSION}/${tarball}.sha256"; \
    (cd /tmp && sha256sum -c "${tarball}.sha256"); \
    mkdir -p /tmp/cigen-dist; \
    tar -xzf "/tmp/${tarball}" -C /tmp/cigen-dist; \
    install -m0755 /tmp/cigen-dist/cigen /usr/local/bin/cigen; \
    mkdir -p "$CIGEN_PLUGIN_DIR"; \
    install -m0755 /tmp/cigen-dist/cigen-provider-circleci "$CIGEN_PLUGIN_DIR"/cigen-provider-circleci; \
    install -m0755 /tmp/cigen-dist/cigen-provider-github "$CIGEN_PLUGIN_DIR"/cigen-provider-github; \
    rm -rf /tmp/${tarball} /tmp/${tarball}.sha256 /tmp/cigen-dist

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
