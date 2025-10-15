#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image_tag="docspring/cigen-ci-runner:latest"
# Default to arm64 for local Apple Silicon workflows; override with
# CI_RUNNER_PLATFORM=linux/amd64 when we need an x86_64 variant.
platform="${CI_RUNNER_PLATFORM:-linux/arm64}"

echo "Building ${image_tag} (${platform}) from docker/ci-runner/Dockerfile"
docker buildx build \
  --platform "${platform}" \
  --file "${root_dir}/docker/ci-runner/Dockerfile" \
  --tag "${image_tag}" \
  --load \
  "${root_dir}"

echo "\nImage ${image_tag} built locally. Push to Docker Hub when ready:"
echo "  docker push ${image_tag}"
