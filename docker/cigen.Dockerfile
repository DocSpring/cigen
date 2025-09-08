FROM cimg/base:stable

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl jq bash git \
 && rm -rf /var/lib/apt/lists/*

ARG CIGEN_VERSION=0.0.0
ARG TARGETARCH

RUN set -e; \
    case "$TARGETARCH" in \
      "amd64") ARCH="amd64" ;; \
      "arm64") ARCH="arm64" ;; \
      *) echo "Unsupported arch: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    echo "Installing cigen v${CIGEN_VERSION} for ${ARCH}"; \
    curl -fsSL -o /tmp/cigen.tar.gz \
      "https://github.com/DocSpring/cigen/releases/download/v${CIGEN_VERSION}/cigen-linux-${ARCH}.tar.gz"; \
    tar -xzf /tmp/cigen.tar.gz -C /usr/local/bin; \
    chmod +x /usr/local/bin/cigen; \
    rm -f /tmp/cigen.tar.gz

ENTRYPOINT ["cigen"]
