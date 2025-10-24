ARG CIGEN_VERSION=0.1.0

FROM debian:bookworm-slim AS runtime

ARG CIGEN_VERSION
ENV CIGEN_VERSION=${CIGEN_VERSION}
ENV CIGEN_PLUGIN_DIR=/usr/local/lib/cigen/plugins

RUN set -euxo pipefail \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
        curl \
        git \
        jq \
        tar \
        unzip \
    && rm -rf /var/lib/apt/lists/*

RUN set -euxo pipefail; \
    arch="$(dpkg --print-architecture)"; \
    case "$arch" in \
        amd64) asset_suffix="linux_amd64" ;; \
        arm64) asset_suffix="linux_arm64" ;; \
        *) echo "Unsupported architecture: ${arch}" >&2; exit 1 ;; \
    esac; \
    tarball="cigen_${CIGEN_VERSION}_${asset_suffix}.tar.gz"; \
    curl -fsSL -o "/tmp/${tarball}" \
      "https://github.com/DocSpring/cigen/releases/download/v${CIGEN_VERSION}/${tarball}"; \
    curl -fsSL -o "/tmp/${tarball}.sha256" \
      "https://github.com/DocSpring/cigen/releases/download/v${CIGEN_VERSION}/${tarball}.sha256"; \
    (cd /tmp && sha256sum -c "${tarball}.sha256"); \
    mkdir -p /tmp/cigen-dist; \
    tar -xzf "/tmp/${tarball}" -C /tmp/cigen-dist; \
    install -m0755 /tmp/cigen-dist/cigen /usr/local/bin/cigen; \
    mkdir -p "${CIGEN_PLUGIN_DIR}"; \
    install -m0755 /tmp/cigen-dist/cigen-provider-circleci "${CIGEN_PLUGIN_DIR}/cigen-provider-circleci"; \
    install -m0755 /tmp/cigen-dist/cigen-provider-github "${CIGEN_PLUGIN_DIR}/cigen-provider-github"; \
    rm -rf /tmp/${tarball} /tmp/${tarball}.sha256 /tmp/cigen-dist

RUN adduser --disabled-password --gecos "" cigen \
    && mkdir -p /workspace \
    && chown -R cigen:cigen /workspace

USER cigen
WORKDIR /workspace

ENTRYPOINT ["/bin/bash"]
