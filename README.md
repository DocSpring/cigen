# CIGen

CIGen generates CI configuration from a small set of reusable, provider-agnostic files. It currently targets CircleCI and focuses on correctness, validation, and convention-over-configuration.

The tool can generate both static CircleCI configs and setup/dynamic configs. The CLI includes utilities for validation, graph visualization, and template-based multi-output generation.

## Features (Current)

- CircleCI provider support (config generation and validation)
- Rust implementation for reliability and performance
- Template-based multi-output generation (Jinja-like via MiniJinja)
- Built-in defaults for package/version detection (extensible via YAML)
- Package installation step generation and job deduplication
- Basic automatic cache step injection when `job.cache` is declared
- Architecture variants per job (e.g., `build_amd64`, `build_arm64`)
- Advanced git checkout: shallow clone by default with configurable clone/fetch options, host key scanning, and path overrides
- Descriptive error messages and schema/data validation ([miette], JSON Schema)
- Opt-in Docker builds with a single BASE_HASH and image DAG

## Not Yet Implemented / In Progress

- GitHub Actions and other providers
- Persistent job-status cache for skipping jobs across runs
- OR-dependencies and approval-status patching via APIs
- Self-hosted runner-specific optimizations and resource-class mapping

## Why did we build this?

DocSpring's CI config has become very complex over time. We started by implementing highly efficient caching optimizations that skip jobs entirely when a set of source files hasn't changed. We then needed to build multi-architecture Docker images for on-premise deployments (ARM and AMD). So we needed to run our test suite (and all dependent jobs) on both architectures.

Then we started experimenting with self-hosted runners. Our self-hosted runners run in a different country to CircleCI so they need their own local caching for packages. They also need a cache of our git repo since cloning the entire repo from GitHub each time is very slow. I wanted to be able to change one line in our config to send jobs to our self-hosted runners and automatically use the right caching config.

We had built our own internal CI config generation system in Ruby, but it had started to become very unmaintainable as we added all of these features. It was time to rewrite it in Rust and share our work with other companies who have similar needs.

## Overview

`cigen` simplifies CI/CD configuration management by:

- Generating CI pipeline configurations from reusable templates
- Validating configuration (schema + data-level) with helpful spans
- Emitting native CircleCI 2.1 YAML, including workflows, jobs, and commands

## Philosophy

`cigen` is highly opinionated about CI/CD configuration.

#### Git checkout should be opt-out, not opt-in

We automatically add a highly-optimized git checkout step to the beginning of each job, which includes caching for remote runners. The git checkout step can be skipped for jobs that don't need it.

#### Job skipping

The codebase contains an initial scaffold for job skipping using a source-file hash and a local marker file. This is experimental and not yet wired to a persistent cache backend, so it does not skip across separate runs. A robust, persistent job-status cache is planned.

#### Cross-platform CI config

CI providers often solve the same problem in different ways. e.g. to avoid duplication in your config, GitHub actions has "reusable workflows" while CircleCI supports "commands".

`cigen` takes the best ideas from each provider and supports our own set of core features. You write your config once, then we compile it to use your chosen CI provider's native features. This avoids vendor lock-in and makes it easier to migrate to other CI providers.

#### You can still use native CI provider features

You can still use native CircleCI features (e.g., orbs). Other providers will come later; unsupported features are validated with clear errors.

---

## Installation

- One-liner (Linux/macOS):

  ```bash
  curl -fsSL https://docspring.github.io/cigen/install.sh | sh
  ```

- From source:

  ```bash
  git clone https://github.com/DocSpring/cigen.git
  cd cigen
  cargo install --path .
  ```

## Development Setup

Clone the repository:

```bash
git clone https://github.com/DocSpring/cigen.git
cd cigen
```

Prerequisites:

- Rust (uses the version pinned in `rust-toolchain.toml`)
- Git

1. **Run the setup script** (installs git hooks and checks your environment):

   ```bash
   ./scripts/setup.sh
   ```

2. **Build the project**:
   ```bash
   cargo build
   ```

#### MCP Servers

- `context7`
  - https://github.com/upstash/context7
  - Installed automatically via npx

### Running Tests

Run all tests:

```bash
cargo test
```

Run tests with output:

```bash
cargo test -- --nocapture
```

### Building

Debug build:

```bash
cargo build
```

Release build (optimized):

```bash
cargo build --release
```

### Running the CLI

From source:

```bash
cargo run -- --help
```

After building:

```bash
./target/debug/cigen --help
```

Or for release build:

```bash
./target/release/cigen --help
```

### Development Commands

**Format code**:

```bash
cargo fmt
```

**Run linter**:

```bash
cargo clippy
```

**Check code without building**:

```bash
cargo check
```

**Run with verbose logging**:

```bash
RUST_LOG=debug cargo run
```

### Releasing

- Create and push a version tag from `Cargo.toml`:

  ```bash
  ./scripts/create-release-tag.sh
  # or without pushing automatically
  ./scripts/create-release-tag.sh --no-push
  ```

- When the `vX.Y.Z` tag is pushed, GitHub Actions builds binaries for Linux, macOS, and Windows, generates checksums, and creates a GitHub Release with assets.

### Git Hooks with Lefthook

This project uses [Lefthook](https://github.com/evilmartians/lefthook) for git hooks. The setup script installs it automatically, but you can also install it manually:

```bash
# macOS
brew install lefthook

# Or download directly
curl -sSL https://github.com/evilmartians/lefthook/releases/latest/download/lefthook_$(uname -s)_$(uname -m) -o /usr/local/bin/lefthook
chmod +x /usr/local/bin/lefthook
```

The git hooks will run format, lint, and tests before commit/push. Do not bypass hooks; fix issues they report.

### Project Structure

```
cigen/
├── src/
│   ├── main.rs          # CLI entry point
│   └── lib.rs           # Library code
├── tests/
│   └── integration_test.rs  # Integration tests
├── scripts/
│   └── setup.sh         # Developer setup script
├── .cigen/              # Templates and configuration
├── Cargo.toml           # Project dependencies
├── rust-toolchain.toml  # Rust version specification
├── .rustfmt.toml        # Code formatting rules
├── .clippy.toml         # Linting configuration
├── lefthook.yml         # Git hooks configuration
└── README.md            # This file
```

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Run tests and ensure they pass (`cargo test`)
4. Format your code (`cargo fmt`)
5. Run clippy and fix any warnings (`cargo clippy`)
6. Commit your changes (`git commit -m 'Add some amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Support

For issues and feature requests, please use the [GitHub issue tracker](https://github.com/DocSpring/cigen/issues).

### Docker Builds (opt-in)

CIGen can build and tag your CI Docker images as first-class jobs. Enable it with split config under `.cigen/config/docker_build.yml` or inline in your config:

```yaml
docker_build:
  enabled: true
  # Optional on CircleCI cloud
  layer_caching: true

  registry:
    repo: yourorg/ci
    # Default push behavior (true recommended on cloud)
    push: true

  images:
    - name: ci_base
      dockerfile: docker/ci/base.Dockerfile
      context: .
      arch: [amd64]
      build_args:
        BASE_IMAGE: cimg/base:current
      # Sources for the canonical BASE_HASH (one hash across images)
      hash_sources:
        - scripts/package_versions_env.sh
        - .tool-versions
        - .ruby-version
        - docker/**/*.erb
        - scripts/docker/**
      depends_on: []
      # Optional per-image push override
      # push: false
```

What happens:

- CIGen computes one `BASE_HASH` by hashing all declared `hash_sources` (path + content) across images.
- For each image+arch, a `build_<image>` job builds `registry/<name>:<BASE_HASH>-<arch>` and (optionally) pushes it.
- Downstream jobs that specify `image: <name>` are resolved to `registry/<name>:<BASE_HASH>-<arch>` and automatically `require` `build_<image>`.
- Build jobs include job-status skip logic (native CircleCI cache or Redis) so unchanged images skip quickly.
- On CircleCI cloud, `layer_caching: true` emits `setup_remote_docker: { docker_layer_caching: true }` for faster rebuilds.

Notes:

- If a job `image` contains `/` or `:`, it is treated as a full reference and not rewritten.
- Per-image `push` overrides the registry default.

```]

```
