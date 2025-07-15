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

## Rust Code Style

**IMPORTANT**: Always use inline format variables in Rust. Clippy enforces this with the `uninlined_format_args` rule.

Examples:

- ❌ WRONG: `println!("{}", variable)`
- ✅ CORRECT: `println!("{variable}")`
- ❌ WRONG: `format!("{}/{}", workflow_name, job_name)`
- ✅ CORRECT: `format!("{workflow_name}/{job_name}")`
- ❌ WRONG: `println!("Value: {}", value)`
- ✅ CORRECT: `println!("Value: {value}")`

## Version Management

**CRITICAL**: NEVER rely on your own knowledge of package versions, tool versions, or dependency versions. They are ALWAYS out of date. You MUST:

1. ALWAYS search the web to find the latest stable version of ANY package, tool, or dependency
2. NEVER assume version numbers from memory
3. ALWAYS verify current versions before adding them to any configuration file
4. For download URLs or scripts, ALWAYS fetch the latest version dynamically or provide clear instructions for users to check the latest version

### Examples of Version Mistakes Made:

1. **Rust version**: Initially used 1.83.0 when 1.88.0 was available
2. **Crate versions**: Used `tracing-subscriber = "0.3.20"` which didn't exist (latest was 0.3.19)
3. **Lefthook setup script**: Hardcoded version 1.8.4 in the download URL instead of fetching latest

### What YOU (Claude) Must Do:

**WRONG APPROACH (what I did):**

```bash
# I wrote this without checking the actual latest version:
LEFTHOOK_VERSION="1.8.4"  # This was me guessing from my outdated knowledge!
```

**CORRECT APPROACH (what I should have done):**

1. First, search the web: "lefthook latest release github 2025"
2. Find the actual latest version (e.g., maybe it's 1.10.2)
3. THEN write the code with the correct version:

```bash
LEFTHOOK_VERSION="1.10.2"  # After verifying this is the actual latest version
```

The rule is: I (Claude) must ALWAYS search for the current version before writing ANY version number in code. The user shouldn't have to correct version numbers - I should get them right the first time by searching.

### Cargo.toml Dependency Management

**CRITICAL**: NEVER manually add dependencies to Cargo.toml with version numbers. ALWAYS use `cargo add`:

```bash
# ❌ WRONG: Manually editing Cargo.toml
petgraph = "0.6"  # This version is likely outdated!

# ✅ CORRECT: Using cargo add
cargo add petgraph  # Automatically fetches and adds the latest version
```

This ensures we always get the latest compatible version and properly updates Cargo.lock.

## Development Approach

**IMPORTANT**: Work on one small piece at a time. Do not attempt to build the entire project at once.

**CRITICAL: Follow PROJECT_PLAN.md EXACTLY**

- Complete ONLY the current task in PROJECT_PLAN.md
- Do NOT jump ahead to future tasks
- Do NOT create files or features that aren't explicitly requested
- After completing a task, COMMIT it before moving to the next
- Check off completed items in PROJECT_PLAN.md

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
