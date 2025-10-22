# Cigen Development Notes

## Design Decisions

### CircleCI Version Support

- **Decision**: Only support the latest CircleCI config version (2.1)
- **Rationale**: There's no benefit in supporting older config versions. This simplifies the codebase and encourages users to adopt current best practices.
- **Date**: December 2024

### Provider Architecture

- Providers are responsible for translating the internal object model to CI-specific configuration formats
- Each provider lives in its own module under `src/providers/`
- Providers implement a common trait to ensure consistency

### CI Runner Image

- Added `docker/ci-runner/` for the base image we use in GitHub Actions `container` jobs
- Image extends `rust:latest` and pre-installs Node/npm, protobuf, Python, and Rust components
- Build locally with `./scripts/build-ci-runner.sh` (tags as `docspring/cigen-ci-runner:latest`)
- `act` picks up the local image automatically; push to Docker Hub before relying on it in hosted CI
- All jobs in `.cigen/workflows/ci/` now default to this image so steps stay fast and reproducible
- Rust toolchain lives in `/usr/local/{rustup,cargo}` with permissive ownership so the non-root runner reuses the cached components without redownloading each job

### Self-Referential Job Hashing

- `cigen hash --job <id>` now loads `.cigen/config.yml` (or split config) to determine source groups
- Hashing uses `git ls-files` to gather tracked files and de-dupes shared patterns across groups
- Each job hash also fingerprints the canonical job definition and workflow metadata, so config-only changes bust caches
- GitHub provider emits a single CLI call (`./.cigen/bin/cigen hash --job ...`) instead of inlining pattern lists per job

## Current Status (January 2025)

- GitHub Actions is now generated via the `cigen-provider-github` plugin. CircleCI support has not yet been ported onto the plugin system.
- The CI pipeline builds `cigen` once (`build_cigen` job) and shares the binary across fmt/clippy/test for skip-cache hashing.
- Skip cache works in GitHub Actions via `actions/cache`; the preflight hook is still a TODO for cross-provider reuse.
- The prior workspace-specific integration has been removed; future workspace awareness will come from Turborepo metadata once the module system lands.
- Plugin manager handles spawn/handshake/send/receive, but plugin discovery and detect/plan phases remain stubs.
