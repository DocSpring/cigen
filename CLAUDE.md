# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`cigen` is a Rust CLI tool that generates CI pipeline configurations from templates. It integrates with Nx monorepo tooling and supports multiple CI providers starting with CircleCI.

See `PRD.txt` for the complete product requirements and specifications.

## Build Commands

```bash
# Build the project
cargo build

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy
```

## Development Approach

**IMPORTANT**: Work on one small piece at a time. Do not attempt to build the entire project at once.

1. Set up foundation first (Cargo.toml, basic CLI with --help and --version)
2. Establish testing infrastructure before adding features
3. For each new feature:
   - Write tests first
   - Implement minimal code to pass tests
   - Run `cargo fmt` and `cargo clippy`
   - Verify tests pass
   - Commit to git
   - Only then move to the next feature

Small, verifiable chunks prevent errors and ensure steady progress.

## Use Our Own Tool

The goal is to eventually become 'self-hosting' for our own CI pipeline on GitHub Actions. We must have `nx.json` and `project.json` file in the root of the repository, a `.cigen/` directory, and a `.cigen/cigen.yml` file.

We will start by hand-writing our own GitHub Actions workflow files, but eventually migrate to using `cigen` to generate our CI configuration.

## Key Concepts

- The tool reads Nx `project.json` files to understand project dependencies and file groups
- Templates and configuration live in the `.cigen/` directory
- The tool supports plugin-based cache backends and CI provider emitters
