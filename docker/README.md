# Docker Images

This directory contains Dockerfiles for images we use during development and CI.

## Layout

- `ci-runner/` – Base image for GitHub Actions jobs that run cigen’s own
  pipelines. It layers tooling we rely on (Node/npm for JavaScript actions,
  `protoc` for gRPC codegen, Python utilities, etc.) on top of the official
  `rust:latest` image. We build this locally for fast `act` runs and will push
  it to Docker Hub once it is finalized.

Future images (e.g. the published `cigen` runtime container) should each live in
their own subdirectory alongside `ci-runner/`.
