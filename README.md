# CIGen

CIGen makes your CI configuration files more maintainable. Reduce duplication, avoid vendor lock-in.

This tool can be used to build both static config and [dynamic config for CircleCI](https://circleci.com/docs/dynamic-config/). The CLI includes a file hashing feature that can be used during the initial setup workflow to skip jobs when no files have changed.

## Features

- Written in Rust
- First-class support for caching with configurable backends
  - Automatically adds OS, version, and architecture to cache keys
- First-class support for running jobs on multiple architectures and self-hosted runners
- Automatic git checkout with extra caching support for self-hosted runners
- Intelligent job skipping based on file changes
- Automatic job dependencies with cache restoration
- Powerful templating engine ([Tera](https://github.com/Keats/tera))
- Beautiful and descriptive error messages ([miette](https://docs.rs/miette/latest/miette/))

## Why did we build this?

DocSpring's CI config has become very complex over time. We started by implementing highly efficient caching optimizations that skip jobs entirely when a set of source files hasn't changed. We then needed to build multi-architecture Docker images for on-premise deployments (ARM and AMD). So we needed to run our test suite (and all dependent jobs) on both architectures.

Then we started experimenting with self-hosted runners. Our self-hosted runners run in a different country to CircleCI so they need their own local caching for packages. They also need a cache of our git repo since cloning the entire repo from GitHub each time is very slow. I wanted to be able to change one line in our config to send jobs to our self-hosted runners and automatically use the right caching config.

We had built our own internal CI config generation system in Ruby, but it had started to become very unmaintainable as we added all of these features. It was time to rewrite it in Rust and share our work with other companies who have similar needs.

## Overview

`cigen` simplifies CI/CD configuration management by:

- Generating CI pipeline configurations from reusable templates
- Integrating with Nx monorepo tooling to understand project dependencies
- Supporting multiple CI providers (starting with CircleCI)
- Providing plugin-based architecture for cache backends and CI providers

## Philosophy

`cigen` is highly opinionated about CI/CD configuration.

#### Git checkout should be opt-out, not opt-in

We automatically add a highly-optimized git checkout step to the beginning of each job, which includes caching for remote runners. The git checkout step can be skipped for jobs that don't need it.

#### Jobs should be skipped if nothing has changed

Most CI providers only support caching as a second-class feature - something you add as a "step" during your job. `cigen` makes caching an integral part of your CI config. Every job MUST provide a list of file patterns. If none of those files have changed, the job is skipped and the existing cache is used. We inject all of the caching steps automatically.

#### Cross-platform CI config

CI providers often solve the same problem in different ways. e.g. to avoid duplication in your config, GitHub actions has "reusable workflows" while CircleCI supports "commands".

`cigen` takes the best ideas from each provider and supports our own set of core features. You write your config once, then we compile it to use your chosen CI provider's native features. This avoids vendor lock-in and makes it easier to migrate to other CI providers.

#### You can still use native CI provider features

You can still write a step that uses GitHub's "Actions" or CircleCI's "orbs". (We'll just raise a validation error if you try to use an "orb" on GitHub actions.)

---

## Development Setup

Clone the repository:

```bash
git clone https://github.com/DocSpring/cigen.git
cd cigen
```

Prerequisites:

- Rust (will automatically use 1.88.0 via `rust-toolchain.toml`)
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

### Git Hooks with Lefthook

This project uses [Lefthook](https://github.com/evilmartians/lefthook) for git hooks. The setup script installs it automatically, but you can also install it manually:

```bash
# macOS
brew install lefthook

# Or download directly
curl -sSL https://github.com/evilmartians/lefthook/releases/latest/download/lefthook_$(uname -s)_$(uname -m) -o /usr/local/bin/lefthook
chmod +x /usr/local/bin/lefthook
```

The git hooks will:

- **pre-commit**: Run `cargo fmt --check`, `cargo clippy`, and `cargo test`
- **pre-push**: Run full checks including `cargo check`

To skip hooks temporarily: `git commit --no-verify`

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
├── .cigen/              # Templates and configuration (future)
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
