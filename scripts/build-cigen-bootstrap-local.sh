#!/bin/sh
set -eu

IMAGE_TAG="${1:-docspringcom/cigen:latest-amd64}"
DOCKERFILE="docker/cigen-bootstrap/Dockerfile.local"

if [ ! -f "$DOCKERFILE" ]; then
  echo "Error: ${DOCKERFILE} not found" >&2
  exit 1
fi

echo "Building local cigen bootstrap image (linux/amd64)..."

DOCKER_DEFAULT_PLATFORM=linux/amd64 \
  docker build \
    -f "$DOCKERFILE" \
    -t "$IMAGE_TAG" \
    .

echo "Built ${IMAGE_TAG}"
