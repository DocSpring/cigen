# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## CRITICAL: Read Notes Before Starting

**BEFORE beginning ANY work session and AFTER any context reset/compaction:**

1. Read ALL .md files in the `notes/` directory
2. These are YOUR working notes - keep them up to date
3. Remove any inaccurate or outdated information
4. Add new learnings and discoveries
5. These notes contain critical architectural decisions and implementation details

Key notes files:

- `notes/CACHING.md` - Automatic cache injection system, NO manual cache steps
- `notes/CONFIG_FORMAT.md` - Configuration structure and conventions
- `notes/TEMPLATING.md` - Template engine details and syntax
- `notes/REQUIREMENTS.md` - Core requirements and architecture
- `notes/PROJECT_PLAN.md` - Implementation roadmap

## CRITICAL: Git Commit Rules

**NEVER EVER use `--no-verify` flag when committing**. This bypasses important checks and is lazy and unprofessional. If pre-commit hooks fail:

1. FIX the issues that the hooks are reporting
2. Run the hooks locally to verify they pass
3. Only commit when ALL checks pass cleanly
4. NO EXCEPTIONS to this rule

Using `--no-verify` is a sign of poor craftsmanship and will not be tolerated.

## Project Overview

`cigen` is a Rust CLI tool that generates CI pipeline configurations from templates. It integrates with Nx monorepo tooling and supports multiple CI providers starting with CircleCI.

See `PRD.txt` for the complete product requirements and specifications.

## Shell Script Compatibility

**CRITICAL**: When writing shell scripts or commands (especially for version detection), ensure full compatibility across all systems:

1. **Use POSIX-compliant features only** - No GNU-specific flags or modern bash features
2. **Test compatibility** with both BSD (macOS) and GNU (Linux) versions of tools
3. **Avoid these common incompatibilities**:
   - `grep -P` (PCRE) - Not available on BSD grep
   - `grep -o` with complex patterns - Behavior differs between implementations
   - `sed -i` without backup extension on macOS
   - Modern bash features like `[[` conditions or arrays

4. **Preferred patterns**:
   - Use `grep -E` instead of `grep -P` for extended regex
   - Use simple `grep | grep` chains instead of complex single patterns
   - Always provide backup extension for `sed -i`: `sed -i.bak` (then delete .bak file)
   - Use `/bin/sh` compatible syntax, not bash-specific

5. **Example of compatible version extraction**:

   ```bash
   # Good - works everywhere
   ruby --version | grep -o '[0-9]*\.[0-9]*\.[0-9]*' | head -1

   # Bad - GNU grep only
   ruby --version | grep -oP '\d+\.\d+\.\d+'
   ```

## Build Commands

```bash
# Build the project
cargo build

# Run tests
cargo test

# Format code
cargo fmt

# Lint code (use the same flags as the git hook)
cargo clippy --all-targets --all-features -- -D warnings

# Test validation on the example
cargo run -- --config examples/circleci_rails/ validate

# Test generation on the example
cargo run -- --config examples/circleci_rails/ generate
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

## Code Organization

**CRITICAL**: Keep files small and modular. As soon as a file approaches 200-300 lines, break it up into modules and smaller files. DO NOT wait for the user to remind you. Proactively refactor large files into:

- Separate modules for distinct functionality
- Helper functions in their own files
- Traits and implementations in separate files
- Tests in separate test modules

This keeps the codebase maintainable and easier to understand.

## Development Approach

**CRITICAL**: NEVER implement temporary solutions or workarounds. NEVER say "for now" or "this is not ideal but". Always implement the proper, clean, architectural solution from the start. If something needs to be done right, do it right the first time.

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

## Before Marking Tasks Complete

**CRITICAL**: Before marking ANY task as complete in your todo list, you MUST:

1. Run `cargo test` to ensure all tests pass
2. Run `cargo clippy --all-targets --all-features -- -D warnings` to check for linting issues
3. Fix any failing tests or clippy warnings
4. Only mark the task as complete after tests pass and clippy is clean

This ensures code quality and prevents accumulating technical debt.

## Error Handling

**CRITICAL**: NEVER silently fail or provide dummy/placeholder behavior:

1. **NO DEFAULT VALUES**: Never return placeholder text like "No command specified" or empty defaults when data is missing
2. **FAIL FAST**: Use `bail!()` or `panic!()` with descriptive error messages when encountering invalid states
3. **EXPLICIT ERRORS**: Always provide clear, actionable error messages that explain what went wrong
4. **VALIDATE EARLY**: Check preconditions and validate data as early as possible
5. **PRESERVE UNKNOWN DATA**: When encountering unknown configurations or step types, preserve them as-is rather than dropping them or converting to defaults

Bad example:

```rust
// NEVER DO THIS
command: "echo 'No command specified'".to_string(),
```

Good example:

```rust
// DO THIS INSTEAD
miette::bail!("Invalid step configuration: missing required 'command' field")
```

## CircleCI Reference Implementation

**IMPORTANT**: The source of truth for all base features is in `circleci_config_reference/src/*`. This contains the ERB templates from our production Ruby implementation. All features shown there must be supported.

You can also reference `/Users/ndbroadbent/code/docspring/lib/tools/generate_circle_ci_config.rb` to understand how the Ruby script worked.

**Testing Strategy**: Add comprehensive test cases as you implement features, based on what you find in the reference implementation. Every feature from the reference should have corresponding tests.

## DocSpring Integration vs Public Examples

**CRITICAL SECURITY DISTINCTION**:

### Public Examples (`examples/`)

- **PUBLIC MIT LICENSED** code visible to all users
- Serves as demonstration of complex, production-ready CI pipeline patterns
- Must contain NO internal DocSpring information, secrets, or sensitive details
- Use generic company names, sanitized configurations, example data only
- This is what users see to learn how to use cigen

### Private DocSpring Work (`./docspring/`)

- **PRIVATE INTERNAL** DocSpring monorepo (symlinked)
- Contains actual DocSpring production CI configuration
- Work in `./docspring/.cigen/` directory for real DocSpring conversion
- Source jobs are in `./docspring/.circleci/src/ci_jobs/` and `./docspring/.circleci/src/deploy_jobs/`
- Convert ERB templates to cigen YAML format in `./docspring/.cigen/workflows/`
- Use DocSpring's actual production requirements

**NEVER COPY INTERNAL DOCSPRING DETAILS TO PUBLIC EXAMPLES**

## Use Our Own Tool

The goal is to eventually become 'self-hosting' for our own CI pipeline on GitHub Actions. We must have `nx.json` and `project.json` file in the root of the repository, a `.cigen/` directory, and a `.cigen/cigen.yml` file.

We will start by hand-writing our own GitHub Actions workflow files, but eventually migrate to using `cigen` to generate our CI configuration.

## DocSpring Configuration Location

**CRITICAL**: When working on DocSpring configuration conversion, the `.cigen/` directory is located in `/Users/ndbroadbent/code/cigen/docspring/.cigen/` (via the symlinked monorepo), NOT in the main cigen repository. This is on the `nathan/cigen-config` branch of the DocSpring monorepo.

The DocSpring configuration uses cigen to replace their existing ERB-based CircleCI configuration system. All job definitions, commands, and templates for DocSpring should be created in `/Users/ndbroadbent/code/cigen/docspring/.cigen/`.

## Key Concepts

- The tool reads Nx `project.json` files to understand project dependencies and file groups
- Templates and configuration live in the `.cigen/` directory
- The tool supports plugin-based cache backends and CI provider emitters
