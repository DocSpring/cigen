# cigen

A CLI tool that generates CI pipeline configurations from templates, with built-in support for Nx monorepos.

## Overview

`cigen` simplifies CI/CD configuration management by:

- Generating CI pipeline configurations from reusable templates
- Integrating with Nx monorepo tooling to understand project dependencies
- Supporting multiple CI providers (starting with CircleCI)
- Providing plugin-based architecture for cache backends and CI providers

## Prerequisites

- Rust (will automatically use 1.88.0 via `rust-toolchain.toml`)
- Git

## Getting Started

### Installation

Clone the repository:

```bash
git clone https://github.com/DocSpring/cigen.git
cd cigen
```

### Development Setup

1. **Install Rust** (if not already installed):

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

   NOTE: Homebrew is not recommended for installing Rust.

2. **The project will automatically use the correct Rust version**:

   When you run any `cargo` command, it will automatically download and use Rust 1.88.0 with rustfmt and clippy included (configured in `rust-toolchain.toml`).

3. **Run the setup script** (installs git hooks and checks your environment):
   ```bash
   ./scripts/setup.sh
   ```

4. **Build the project**:
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
