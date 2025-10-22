# Self-Hosting CIGen on GitHub Actions

## Overview

This document outlines the complete plan to:

1. Add GitHub Actions as a supported provider in cigen
2. Make cigen self-hosting (use cigen to generate its own GitHub Actions CI)
3. Support self-hosted runners with in-house caching
4. Implement job skipping logic for GitHub Actions

## Current State

- **CI Platform**: GitHub Actions (generated workflows: `ci.yml`, `docs.yml`, `release.yml`)
- **Workflow generation**: `.cigen/` split config rendered through the GitHub provider plugin — the old workspace-specific scaffolding has been removed
- **Caching**: Uses the skip-cache flow (via `actions/cache@v4`) plus a small build artifact for the `cigen` binary
- **Package management**: Rust toolchain managed through rustup inside the `rust:latest` container; Node/pnpm steps only run when `act` needs them
- **Plugin architecture**: Core spawns `cigen-provider-github` during `cargo run -- generate --file .cigen`

## Goals

1. **GitHub Actions Provider**: Fully implement `github-actions` provider alongside CircleCI
2. **Self-Hosting**: Use cigen to generate cigen's own CI workflows
3. **Automatic Package Installation**: Leverage cigen's package management for Rust, Node.js
4. **Intelligent Caching**: Use cigen's smart caching for Rust cargo and Node.js packages
5. **Job Skipping**: Implement source-file-based job skipping for GitHub Actions
6. **Self-Hosted Runner Support**: Enable switching to self-hosted runners with one config change
7. **Custom Cache Backend**: Support in-house caching (S3/MinIO) for self-hosted runners
8. **Turborepo Workspace Awareness**: Replace the old monorepo assumptions with Turborepo project graph ingestion once the module system lands

## Architecture Comparison: CircleCI vs GitHub Actions

### Dynamic Workflow Generation

**CircleCI:**

- **Setup workflow** with `setup: true` runs first
- Can use continuation API to generate and launch a completely new workflow YAML
- Enables **complete job skipping** - jobs never start if files unchanged
- Setup workflow computes file hashes, checks cache, filters jobs, posts new workflow

**GitHub Actions:**

- **No runtime workflow generation** - workflow structure is fixed when pipeline starts
- Cannot use continuation pattern to skip entire jobs
- Jobs must start, but can **exit early** based on conditions

**Cigen Strategy:**
Per the philosophy in `PHILOSOPHY.md`: "If a CI platform doesn't support a feature we need, we build it ourselves."

For GitHub Actions:

- Jobs **will spin up** (unavoidable limitation)
- Inject early-exit step at job start that checks source file hashes
- If hash exists in skip cache, exit early with success (saves 95%+ of job time)
- On successful completion, record hash in skip cache
- Use same SHA-256 hash logic as CircleCI for consistency

### Job Skip Implementation Pattern

```yaml
# What cigen generates for a job with source_files
jobs:
  ruby_lint:
    runs-on: ubuntu-latest
    steps:
      # INJECTED: Check skip cache
      - name: Check if job should be skipped
        id: skip_check
        run: |
          # Compute JOB_HASH from source files (same logic as CircleCI)
          JOB_HASH=$(compute_source_hash ...)

          # Check if hash exists in cache (GitHub Actions cache or custom backend)
          if cache_exists "job-skip-$JOB_HASH"; then
            echo "Files unchanged since last successful run"
            echo "skip=true" >> $GITHUB_OUTPUT
            exit 0
          fi
          echo "skip=false" >> $GITHUB_OUTPUT
          echo "JOB_HASH=$JOB_HASH" >> $GITHUB_ENV

      # INJECTED: Early exit if skipping
      - name: Exit if skipping
        if: steps.skip_check.outputs.skip == 'true'
        run: exit 0

      # User's actual job steps (only run if not skipped)
      - uses: actions/checkout@v4
        if: steps.skip_check.outputs.skip != 'true'

      - name: Run RuboCop
        if: steps.skip_check.outputs.skip != 'true'
        run: bundle exec rubocop

      # INJECTED: Record successful completion
      - name: Record job completion
        if: steps.skip_check.outputs.skip != 'true' && success()
        run: |
          # Save marker to cache with key "job-skip-$JOB_HASH"
          cache_save "job-skip-$JOB_HASH"
```

**Key Differences from CircleCI:**

- CircleCI: Setup filters jobs before they start (via continuation)
- GitHub Actions: Jobs start, check early, exit if skipped
- Both use same file hash logic for consistency
- GitHub approach wastes ~5-30 seconds spinning up container, but still massive savings

### Conditional Execution

**CircleCI:**

- Limited built-in conditional support
- Cigen implements OR dependencies via automated approval shim jobs
- Uses pipeline parameters for workflow selection

**GitHub Actions:**

- Rich `if` expressions on jobs and steps
- Supports `needs.job.result`, `needs.job.outputs.var`
- Can check multiple conditions with `&&`, `||`
- Built-in functions: `success()`, `failure()`, `always()`, `cancelled()`

**Cigen Mapping:**

```yaml
# CIGen config
jobs:
  deploy:
    requires_any: [approval_staging, approval_prod]

# GitHub Actions output (no shim jobs needed!)
jobs:
  deploy:
    if: |
      needs.approval_staging.result == 'success' ||
      needs.approval_prod.result == 'success'
    needs: [approval_staging, approval_prod]
```

### Matrix Builds

**CircleCI:**

- No native matrix support
- Cigen generates separate jobs with architecture suffixes
- Example: `install_gems_amd64`, `install_gems_arm64`

**GitHub Actions:**

- Native `strategy.matrix` support
- Can be static or dynamically generated
- Automatic job naming with matrix values

**Cigen Mapping:**

```yaml
# CIGen config
jobs:
  test:
    architectures: [amd64, arm64]
    matrix:
      ruby: ['3.2', '3.3']

# GitHub Actions output
jobs:
  test:
    strategy:
      matrix:
        arch: [amd64, arm64]
        ruby: ['3.2', '3.3']
    runs-on: ubuntu-latest
    steps:
      - run: echo "Testing Ruby ${{ matrix.ruby }} on ${{ matrix.arch }}"
```

### Caching

**CircleCI:**

- `restore_cache` / `save_cache` steps
- Manual key construction: `v1-gems-{{ checksum "Gemfile.lock" }}`
- Automatic platform prefix not included

**GitHub Actions:**

- `actions/cache@v4` action
- Keys: `key` (exact match) and `restore-keys` (prefix fallback)
- No automatic platform/arch prefix

**Cigen Strategy:**
Both providers lack automatic platform/arch/version prefixing. Cigen's cache system provides this automatically:

```yaml
# CIGen config
cache: gems  # Uses built-in gems cache definition

# Generated cache key (both providers):
# linux-ubuntu22.04-amd64-gems-ruby3.3.0-bundler2.5.6-abc123def456

# CircleCI output
- restore_cache:
    keys:
      - linux-ubuntu22.04-amd64-gems-ruby3.3.0-bundler2.5.6-{{ checksum "Gemfile.lock" }}
      - linux-ubuntu22.04-amd64-gems-ruby3.3.0-bundler2.5.6-

# GitHub Actions output
- uses: actions/cache@v4
  with:
    path: |
      vendor/bundle
      .bundle
    key: linux-ubuntu22.04-amd64-gems-ruby3.3.0-bundler2.5.6-${{ hashFiles('Gemfile.lock') }}
    restore-keys: |
      linux-ubuntu22.04-amd64-gems-ruby3.3.0-bundler2.5.6-
```

### Self-Hosted Runners

**CircleCI:**

- Resource classes for runner selection
- Architecture-specific: `medium` vs `arm.medium`

**GitHub Actions:**

- Label-based runner selection
- Default labels: `self-hosted`, `linux`, `x64`, `ARM64`
- Custom labels for capabilities: `gpu`, `high-memory`
- Runner groups for organization

**Cigen Config:**

```yaml
# Single config for both providers
runners:
  cloud:
    resource_class: medium
    cache_backend: native

  self_hosted:
    labels: [self-hosted, linux, x64]
    cache_backend: s3
    s3:
      bucket: my-cache-bucket
      region: us-east-1

# Select runner per workflow/job
jobs:
  test:
    runner: cloud # Uses GitHub-hosted runner with native cache

  build_large:
    runner: self_hosted # Uses self-hosted with S3 cache
```

## Implementation Plan

### Phase 1: GitHub Actions Provider Foundation

**Goal:** Create basic GitHub Actions provider with workflow generation

**Tasks:**

1. **Provider Structure**
   - [ ] Create `src/providers/github_actions/` module
   - [ ] Implement `GitHubActionsProvider` struct
   - [ ] Implement `Provider` trait
   - [ ] Add to `get_provider()` in `src/providers/mod.rs`

2. **Schema Definition**
   - [ ] Create `src/providers/github_actions/schema.rs`
   - [ ] Define GitHub Actions output structures (Workflow, Job, Step, etc.)
   - [ ] Support workflow syntax: `on`, `jobs`, `steps`, `runs-on`, `needs`

3. **Basic Generator**
   - [ ] Create `src/providers/github_actions/generator.rs`
   - [ ] Implement `generate_workflow()` for single workflow
   - [ ] Implement `generate_all()` for multiple workflows
   - [ ] Write YAML to `.github/workflows/{workflow_name}.yml`

4. **Job Compilation**
   - [ ] Map cigen `Job` to GitHub Actions job structure
   - [ ] Handle `runs-on` from runner config
   - [ ] Map `steps` with name/run format
   - [ ] Support `needs` dependencies (AND logic)

5. **Testing**
   - [ ] Unit tests for schema serialization
   - [ ] Integration tests generating simple workflows
   - [ ] Validate output with GitHub Actions schema

**Success Criteria:**

- Can generate a simple GitHub Actions workflow from cigen config
- Workflow validates with GitHub Actions schema
- Basic job dependencies work

### Phase 2: Core Feature Parity

**Goal:** Implement essential features to match CircleCI capabilities

**Tasks:**

1. **Step Types**
   - [ ] `checkout` → `actions/checkout@v4`
   - [ ] `run` → shell commands
   - [ ] `uses` → GitHub Actions from marketplace
   - [ ] Custom actions support

2. **Conditional Execution**
   - [ ] Map `if` conditions from cigen to GitHub Actions syntax
   - [ ] Implement OR dependencies using native `||` conditions
   - [ ] Handle `requires_any` without shim jobs (major advantage!)
   - [ ] Support `success()`, `failure()`, `always()` functions

3. **Matrix Builds**
   - [ ] Generate `strategy.matrix` from cigen architectures
   - [ ] Support custom matrix dimensions (e.g., Ruby versions)
   - [ ] Handle matrix variable substitution in steps

4. **Environment Variables**
   - [ ] Map `environment` to `env` in jobs/steps
   - [ ] Support `${{ env.VAR }}`, `${{ secrets.VAR }}`
   - [ ] Handle `GITHUB_TOKEN` and other built-ins

5. **Services (Containers)**
   - [ ] Map cigen `services` to GitHub Actions `services`
   - [ ] Handle service configuration (ports, env, health checks)
   - [ ] Map service hostnames

**Success Criteria:**

- Can generate workflows with matrices, conditions, services
- OR dependencies work natively without workarounds
- Environment variables and secrets work correctly

### Phase 3: Intelligent Caching

**Goal:** Implement cigen's automatic cache system for GitHub Actions

**Tasks:**

1. **Cache Step Generation**
   - [ ] Create `src/providers/github_actions/cache.rs`
   - [ ] Generate `actions/cache@v4` steps from cache definitions
   - [ ] Inject restore before user steps, save after
   - [ ] Handle cache paths and keys

2. **Cache Key Generation**
   - [ ] Use cigen's cache key format: `{os}-{os_version}-{arch}-{cache_name}-{versions}-{checksum}`
   - [ ] Detect runner OS/arch from `runs-on` config
   - [ ] Generate version detection steps (Ruby, Node.js, Python, etc.)
   - [ ] Compute checksums using `hashFiles()` function

3. **Version Detection**
   - [ ] Read from version files (`.ruby-version`, `.node-version`)
   - [ ] Parse from lock files (Gemfile.lock, package.json)
   - [ ] Run version commands (`ruby --version`, `node --version`)
   - [ ] Store as job environment variables

4. **Built-in Cache Types**
   - [ ] Implement `gems` cache (vendor/bundle, .bundle)
   - [ ] Implement `node_modules` cache
   - [ ] Implement `pip` cache (.venv, ~/.cache/pip)
   - [ ] Implement `cargo` cache (~/.cargo, target/)

5. **Restore Keys (Fallback)**
   - [ ] Generate prefix-based restore keys
   - [ ] Same name + versions, any checksum
   - [ ] Handle cache misses gracefully

**Example Output:**

```yaml
steps:
  # INJECTED: Detect Ruby version
  - name: Detect Ruby version
    id: ruby_version
    run: |
      VERSION=$(cat .ruby-version)
      echo "version=$VERSION" >> $GITHUB_OUTPUT

  # INJECTED: Restore gems cache
  - name: Restore gems cache
    uses: actions/cache@v4
    with:
      path: |
        vendor/bundle
        .bundle
      key: linux-ubuntu22.04-x64-gems-ruby${{ steps.ruby_version.outputs.version }}-${{ hashFiles('Gemfile.lock') }}
      restore-keys: |
        linux-ubuntu22.04-x64-gems-ruby${{ steps.ruby_version.outputs.version }}-

  # User's steps
  - name: Install dependencies
    run: bundle install

  # (save_cache automatically handled by actions/cache post-action)
```

**Success Criteria:**

- Cache keys include platform, arch, versions, checksums
- Restore keys enable prefix matching
- Built-in cache types work for common languages
- Cache hit/miss behaves correctly

### Phase 4: Job Skipping System

**Goal:** Implement source-file-based job skipping for GitHub Actions

**Tasks:**

1. **Skip Cache Backend Abstraction**
   - [ ] Create `src/skip_cache/` module
   - [ ] Define `SkipCacheBackend` trait
   - [ ] Implement `GitHubActionsCache` backend (uses native cache)
   - [ ] Implement `S3SkipCache` backend
   - [ ] Implement `RedisSkipCache` backend

2. **Hash Computation**
   - [ ] Reuse CircleCI's hash computation logic
   - [ ] Generate shell script to compute source file hash
   - [ ] Support glob patterns for source files
   - [ ] Include CI template files automatically (same as CircleCI)

3. **Skip Check Injection**
   - [ ] Inject skip check as first step in job
   - [ ] Compute `JOB_HASH` from source files
   - [ ] Query skip cache backend for hash existence
   - [ ] Set `skip=true/false` output and `JOB_HASH` env var
   - [ ] Early exit if skipping

4. **Conditional Step Execution**
   - [ ] Add `if: steps.skip_check.outputs.skip != 'true'` to all user steps
   - [ ] Ensure checkout only runs if not skipping
   - [ ] Ensure cache restore/save only runs if not skipping

5. **Completion Recording**
   - [ ] Inject completion step at end of job
   - [ ] Only runs on success: `if: success() && steps.skip_check.outputs.skip != 'true'`
   - [ ] Record `JOB_HASH` in skip cache backend
   - [ ] Handle architecture-specific hashes

6. **Backend Configuration**
   - [ ] Add skip cache config to cigen config
   - [ ] Support per-runner backend selection
   - [ ] GitHub-hosted → native cache
   - [ ] Self-hosted → S3/Redis/MinIO

**Example Configuration:**

```yaml
# .cigen/config.yml
provider: github-actions

skip_cache:
  default: native # Use GitHub Actions cache
  backends:
    native:
      # No config needed
    s3:
      bucket: cigen-skip-cache
      region: us-east-1
      prefix: skip/

runners:
  cloud:
    skip_cache: native
  self_hosted:
    skip_cache: s3
```

**Example Generated Job:**

```yaml
jobs:
  ruby_lint:
    runs-on: ubuntu-latest
    steps:
      # INJECTED: Skip check
      - name: Check if job should be skipped
        id: skip_check
        run: |
          # Compute hash from source files
          TEMP_HASH_FILE="/tmp/source_files_for_hash"
          rm -f "$TEMP_HASH_FILE"

          for pattern in "**/*.rb" "Gemfile*" ".ruby-version"; do
            find . -path "$pattern" -type f >> "$TEMP_HASH_FILE" || true
          done

          if [ -f "$TEMP_HASH_FILE" ]; then
            JOB_HASH=$(sort "$TEMP_HASH_FILE" | xargs sha256sum | sha256sum | cut -d' ' -f1)
          else
            JOB_HASH="empty"
          fi

          echo "JOB_HASH=$JOB_HASH" >> $GITHUB_ENV
          echo "job_hash=$JOB_HASH" >> $GITHUB_OUTPUT

          # Check if hash exists in cache
          # (This is a placeholder - actual implementation depends on backend)
          if cache_check "job-skip-linux-x64-ruby_lint-$JOB_HASH"; then
            echo "Files unchanged, skipping job"
            echo "skip=true" >> $GITHUB_OUTPUT
          else
            echo "skip=false" >> $GITHUB_OUTPUT
          fi

      - name: Early exit if skipping
        if: steps.skip_check.outputs.skip == 'true'
        run: |
          echo "Job skipped - source files unchanged since last success"
          exit 0

      # All user steps get conditional
      - uses: actions/checkout@v4
        if: steps.skip_check.outputs.skip != 'true'

      - name: Install dependencies
        if: steps.skip_check.outputs.skip != 'true'
        run: bundle install

      - name: Run RuboCop
        if: steps.skip_check.outputs.skip != 'true'
        run: bundle exec rubocop

      # INJECTED: Record completion
      - name: Record job completion
        if: steps.skip_check.outputs.skip != 'true' && success()
        run: |
          # Save to cache with key "job-skip-linux-x64-ruby_lint-$JOB_HASH"
          cache_save "job-skip-linux-x64-ruby_lint-${{ env.JOB_HASH }}"
```

**Success Criteria:**

- Jobs skip when source files unchanged (early exit after ~5-30 seconds)
- Hash computation matches CircleCI for consistency
- Multiple backends supported (native, S3, Redis)
- Per-runner backend configuration works
- Architecture-aware skip cache (amd64 vs arm64)

### Phase 5: Self-Hosted Runner Support

**Goal:** Enable easy switching between GitHub-hosted and self-hosted runners

**Tasks:**

1. **Runner Configuration**
   - [ ] Define runner profiles in config
   - [ ] Support label-based selection
   - [ ] Support runner groups
   - [ ] Map resource classes to runner labels

2. **Custom Labels**
   - [ ] Support custom labels for capabilities (gpu, high-memory)
   - [ ] Generate `runs-on` with label arrays
   - [ ] Handle label + group combinations

3. **Cache Backend Selection**
   - [ ] Map runner profile to cache backend
   - [ ] GitHub-hosted → native `actions/cache`
   - [ ] Self-hosted → S3/MinIO/Redis
   - [ ] Generate appropriate cache commands per backend

4. **S3/MinIO Cache Backend**
   - [ ] Create `src/cache_backends/s3.rs`
   - [ ] Generate AWS CLI commands for cache operations
   - [ ] Support MinIO-compatible endpoints
   - [ ] Handle credentials via environment variables

5. **Redis Cache Backend**
   - [ ] Create `src/cache_backends/redis.rs`
   - [ ] Generate redis-cli commands
   - [ ] Support TTL configuration
   - [ ] Handle connection strings

**Example Configuration:**

```yaml
# .cigen/config.yml
provider: github-actions

runners:
  cloud:
    # GitHub-hosted runner
    runs_on: ubuntu-latest
    cache_backend: native

  self_hosted:
    # Self-hosted runner
    runs_on: [self-hosted, linux, x64]
    cache_backend: s3

  gpu_runner:
    runs_on: [self-hosted, linux, x64, gpu]
    cache_backend: s3

cache_backends:
  native:
    # Uses actions/cache@v4 - no config needed

  s3:
    bucket: my-ci-cache
    region: us-east-1
    endpoint: https://minio.internal:9000 # For MinIO
    access_key: ${AWS_ACCESS_KEY_ID}
    secret_key: ${AWS_SECRET_ACCESS_KEY}

  redis:
    url: redis://cache.internal:6379
    ttl: 604800 # 7 days

# Use different runners per job
jobs:
  test:
    runner: cloud

  build_ml_model:
    runner: gpu_runner
```

**Generated Workflow:**

```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      # Uses actions/cache@v4
      - uses: actions/cache@v4
        with:
          path: vendor/bundle
          key: ...

  build_ml_model:
    runs-on: [self-hosted, linux, x64, gpu]
    steps:
      # Uses custom S3 cache commands
      - name: Restore cache from S3
        run: |
          aws s3 cp s3://my-ci-cache/linux-x64-ml_deps-$CACHE_KEY.tar.gz /tmp/cache.tar.gz || true
          if [ -f /tmp/cache.tar.gz ]; then
            tar -xzf /tmp/cache.tar.gz
          fi

      # ... build steps ...

      - name: Save cache to S3
        run: |
          tar -czf /tmp/cache.tar.gz models/ data/
          aws s3 cp /tmp/cache.tar.gz s3://my-ci-cache/linux-x64-ml_deps-$CACHE_KEY.tar.gz
```

**Success Criteria:**

- Can switch runner with one line of config change
- Cache backend automatically switches with runner
- S3/MinIO backend works for self-hosted runners
- Redis backend works for self-hosted runners
- Labels and groups correctly generate `runs-on`

### Phase 6: Package Management

**Goal:** Automatic package installation for Rust and other languages

**Tasks:**

1. **Rust Package Management**
   - [ ] Detect Rust from `Cargo.toml`
   - [ ] Generate rustup installation if needed
   - [ ] Generate `cargo fetch` step
   - [ ] Support rustfmt, clippy component installation
   - [ ] Cache `~/.cargo` and `target/`

2. **Node.js Package Management**
   - [ ] Detect package manager (npm, yarn, pnpm, bun)
   - [ ] Generate `actions/setup-node@v4` step
   - [ ] Generate pnpm installation if needed
   - [ ] Generate `pnpm install` / `npm ci` / etc.
   - [ ] Cache `node_modules` and package manager store

3. **Ruby Package Management**
   - [ ] Detect Ruby from `.ruby-version`
   - [ ] Generate `ruby/setup-ruby@v1` step
   - [ ] Generate `bundle install` step
   - [ ] Cache `vendor/bundle`

4. **Python Package Management**
   - [ ] Detect Python from `requirements.txt` / `Pipfile`
   - [ ] Generate `actions/setup-python@v4` step
   - [ ] Generate `pip install` step
   - [ ] Cache `.venv` and `~/.cache/pip`

5. **Integration with Cache System**
   - [ ] Package caches use same cigen key format
   - [ ] Version detection from package manager files
   - [ ] Automatic cache injection for package directories

**Example CIGen Config:**

```yaml
# .cigen/workflows/test/jobs/cargo_test.yml
packages:
  - rust
  - node # For future Turborepo/JS workspace tasks

cache:
  - cargo
  - node_modules

steps:
  - name: Run tests
    run: cargo test
```

**Generated Output:**

```yaml
steps:
  # INJECTED: Setup Rust
  - name: Setup Rust
    uses: actions-rs/toolchain@v1
    with:
      toolchain: stable
      components: rustfmt, clippy

  # INJECTED: Setup Node.js
  - name: Setup Node.js
    uses: actions/setup-node@v4
    with:
      node-version-file: .nvmrc

  # INJECTED: Restore cargo cache
  - uses: actions/cache@v4
    with:
      path: |
        ~/.cargo
        target/
      key: linux-ubuntu-x64-cargo-rust1.88.0-${{ hashFiles('Cargo.lock') }}
      restore-keys: linux-ubuntu-x64-cargo-rust1.88.0-

  # INJECTED: Restore node_modules cache
  - uses: actions/cache@v4
    with:
      path: node_modules
      key: linux-ubuntu-x64-node_modules-node20.11.0-pnpm10.0.0-${{ hashFiles('pnpm-lock.yaml') }}
      restore-keys: linux-ubuntu-x64-node_modules-node20.11.0-pnpm10.0.0-

  # INJECTED: Install Rust dependencies
  - name: Download cargo dependencies
    run: cargo fetch --locked

  # INJECTED: Install Node dependencies
  - name: Install pnpm dependencies
    run: pnpm install --frozen-lockfile

  # User's step
  - name: Run tests
    run: cargo test
```

**Success Criteria:**

- Rust projects automatically get rustup, cargo, cache
- Node.js projects automatically get node, pnpm/npm/yarn, cache
- Package installation is idempotent (cached when possible)
- Works with both GitHub-hosted and self-hosted runners

### Phase 7: CIGen Self-Hosting

**Goal:** Use cigen to generate cigen's own GitHub Actions CI

**Tasks:**

1. **Create CIGen Config**
   - [ ] Create `.cigen/config.yml` with provider: github-actions
   - [ ] Define runner profiles (cloud only for now)
   - [ ] Configure Rust and Node.js packages
   - [ ] Define cache for cargo and node_modules

2. **Define Workflows**
   - [ ] Create `.cigen/workflows/ci/` for main CI workflow
   - [ ] Define jobs: format, lint, test, build, integration

3. **Define Jobs**
   - [ ] `format`: cargo fmt --check
   - [ ] `lint`: cargo clippy
   - [ ] `test`: cargo test
   - [ ] `build`: cargo build --release
   - [ ] `integration`: integration tests

4. **Source File Tracking**
   - [ ] Define source file groups (rust, config)
   - [ ] Add source_files to each job for skip cache
   - [ ] Test skip behavior on successive runs

5. **Turborepo Integration (Future)**
   - [ ] Ensure Turbo tasks can invoke Rust targets once the workspace is defined
   - [ ] Add Turborepo outputs to cache definitions
   - [ ] Support running via `turbo run` or direct cargo

6. **Migration**
   - [ ] Generate `.github/workflows/ci.yml` with cigen
   - [ ] Compare with hand-written version
   - [ ] Test generated workflow in CI
   - [ ] Replace hand-written with generated
   - [ ] Update docs

**Example CIGen Config:**

```yaml
# .cigen/config.yml
provider: github-actions

architectures: [x64]

packages:
  rust:
    version_file: rust-toolchain.toml
    components: [rustfmt, clippy]

  node:
    version_file: .nvmrc
    package_manager: pnpm

cache_definitions:
  cargo:
    versions: [rust]
    checksum_sources:
      - Cargo.lock
    paths:
      - ~/.cargo
      - target/

  node_modules:
    versions: [node, pnpm]
    checksum_sources:
      - pnpm-lock.yaml
    paths:
      - node_modules

  turbo_cache:
    checksum_sources:
      - turbo.json
    paths:
      - .turbo

source_file_groups:
  rust:
    - 'src/**/*.rs'
    - 'Cargo.toml'
    - 'Cargo.lock'

  config:
    - '.cigen/**/*'
    - 'turbo.json'
    - 'package.json'

workflows:
  ci:
    on:
      push:
        branches: [main]
      pull_request:
        branches: [main]
```

```yaml
# .cigen/workflows/ci/jobs/format.yml
packages: [rust]
cache: [cargo]
source_files: '@rust'

steps:
  - name: Check formatting
    run: cargo fmt -- --check
```

```yaml
# .cigen/workflows/ci/jobs/lint.yml
packages: [rust]
cache: [cargo]
source_files: '@rust'

steps:
  - name: Run clippy
    run: cargo clippy --all-targets --all-features -- -D warnings
```

```yaml
# .cigen/workflows/ci/jobs/test.yml
packages: [rust, node]
cache: [cargo, node_modules, turbo_cache]
source_files:
  - '@rust'
  - '@config'

steps:
  - name: Run tests
    run: cargo test --all-features
```

**Success Criteria:**

- `cigen generate` produces valid `.github/workflows/ci.yml`
- Generated workflow runs successfully in GitHub Actions
- All jobs pass (format, lint, test, build)
- Caches work correctly (cargo, node_modules, turbo_cache)
- Job skipping works (skip jobs when files unchanged)
- Can commit generated workflow to replace hand-written version

## Testing Strategy

### Unit Tests

- [ ] Schema serialization for GitHub Actions YAML
- [ ] Cache key generation with all variations
- [ ] Conditional expression generation
- [ ] Matrix strategy generation
- [ ] Runner label generation

### Integration Tests

- [ ] Generate simple workflow, validate syntax
- [ ] Generate workflow with matrix, validate
- [ ] Generate workflow with caching, validate
- [ ] Generate multi-job workflow with dependencies
- [ ] Generate workflow with self-hosted runners

### End-to-End Tests

- [ ] Generate cigen's own CI workflow
- [ ] Run generated workflow in actual GitHub Actions
- [ ] Verify job skip cache works
- [ ] Verify package installation works
- [ ] Verify all caches hit/miss correctly

### Validation

- [ ] Use GitHub's workflow schema validation
- [ ] Use `actionlint` for additional validation
- [ ] Test on GitHub-hosted and self-hosted runners

## Configuration Examples

### Simple Project (GitHub-hosted only)

```yaml
# .cigen/config.yml
provider: github-actions

packages: [node]
cache: [node_modules]

workflows:
  ci:
    on:
      push:
        branches: [main]
```

### Multi-Architecture with Self-Hosted

```yaml
# .cigen/config.yml
provider: github-actions

architectures: [x64, ARM64]

runners:
  cloud:
    runs_on: ubuntu-latest
    cache_backend: native

  self_hosted:
    runs_on: [self-hosted, linux, x64]
    cache_backend: s3

cache_backends:
  s3:
    bucket: ci-cache
    region: us-east-1

jobs:
  test:
    runner: cloud
    architectures: [x64, ARM64] # Matrix build

  deploy:
    runner: self_hosted
```

### Complex Monorepo

```yaml
# .cigen/config.yml
provider: github-actions

packages:
  rust:
    components: [rustfmt, clippy]
  node:
    package_manager: pnpm
  python:
    version: '3.12'

cache:
  - cargo
  - node_modules
  - pip

source_file_groups:
  rust: ['src/**/*.rs', 'Cargo.*']
  js: ['client/**/*.{js,ts,tsx}', 'package.json', 'pnpm-lock.yaml']
  python: ['**/*.py', 'requirements.txt']

workflows:
  ci:
    jobs:
      rust_lint:
        source_files: '@rust'
        steps:
          - run: cargo clippy

      js_test:
        source_files: '@js'
        steps:
          - run: pnpm test

      python_lint:
        source_files: '@python'
        steps:
          - run: ruff check
```

## Migration Path

### For Existing GitHub Actions Users

1. **Initial Setup**
   - Install cigen
   - Create `.cigen/config.yml` with `provider: github-actions`
   - Define existing workflows in `.cigen/workflows/`

2. **Incremental Migration**
   - Start with one simple workflow
   - Generate and compare with hand-written version
   - Test generated workflow
   - Gradually migrate remaining workflows

3. **Add Enhancements**
   - Enable job skipping with source_files
   - Use automatic package management
   - Use intelligent caching
   - Add self-hosted runner support

### For CircleCI Users Switching to GitHub Actions

1. **Update Provider**
   - Change `provider: circleci` → `provider: github-actions`
   - Update output path if needed

2. **Adjust Runner Config**
   - Map CircleCI resource classes to GitHub runner labels
   - Configure runner profiles for different job types

3. **Generate and Test**
   - Run `cigen generate`
   - Review generated `.github/workflows/`
   - Test in GitHub Actions
   - Adjust config as needed

4. **Enjoy Native Features**
   - OR dependencies work natively (no shim jobs!)
   - Native matrix builds
   - Rich conditional expressions

## Performance Expectations

### Job Skip Performance

**GitHub-hosted runners:**

- Job startup: ~5-30 seconds
- Hash computation: ~1-5 seconds
- Cache check: ~1-3 seconds
- **Total skip time: ~7-38 seconds** (vs ~3-5 minutes for full job)
- **Savings: 80-95% time reduction**

**Self-hosted runners:**

- Job startup: ~1-5 seconds (already warm)
- Hash computation: ~1-5 seconds
- Cache check (S3): ~0.5-2 seconds
- Cache check (Redis): ~0.1-0.5 seconds
- **Total skip time: ~2-12 seconds**
- **Savings: 90-98% time reduction**

### Cache Hit Performance

**Native GitHub Actions cache:**

- Cache restore: ~5-30 seconds (depends on size)
- Cache save: ~5-30 seconds
- Bandwidth: Limited by GitHub's cache servers

**S3/MinIO cache (self-hosted):**

- Cache restore: ~2-15 seconds (depends on network and size)
- Cache save: ~2-15 seconds
- Bandwidth: Limited by S3/MinIO and network
- Can be faster for large caches on local network

## Future Enhancements

### Phase 8+: Advanced Features

1. **Reusable Workflows**
   - Generate callable workflows with inputs/outputs
   - Support workflow composition

2. **Composite Actions**
   - Generate custom GitHub Actions from cigen commands
   - Publish to marketplace

3. **Deployment Environments**
   - Support GitHub environment protection rules
   - Map to cigen deployment jobs

4. **Concurrency Control**
   - Generate concurrency groups
   - Support queue management

5. **Artifacts**
   - Auto-generate artifact upload/download
   - Track artifacts across jobs

6. **Status Checks**
   - Custom status check reporting
   - Integration with GitHub branch protection

7. **Security Scanning**
   - Integrate GitHub security features
   - CodeQL, dependency scanning

## Open Questions

1. **How to handle GitHub Actions marketplace actions?**
   - Support `uses:` step type in cigen config
   - Map to specific actions with versions
   - Provide cigen wrappers for common patterns

2. **How to handle workflow dispatch inputs?**
   - Add `workflow_dispatch` trigger type
   - Support input definitions
   - Pass inputs to jobs as variables

3. **How to handle secrets in config?**
   - Document secrets must be in GitHub settings
   - Support `${{ secrets.NAME }}` in generated workflows
   - Provide secret validation/checking

4. **How to handle GitHub-specific features?**
   - GITHUB_TOKEN permissions
   - Environments and approvals
   - Deployment protection rules
   - Keep these as optional provider-specific extensions

## Success Metrics

- [ ] CIGen successfully generates its own CI workflow
- [ ] Generated workflow passes all CI checks
- [ ] Job skipping reduces CI time by 80%+ on unchanged code
- [ ] Self-hosted runners work with custom cache backends
- [ ] Can switch between GitHub-hosted and self-hosted with config change
- [ ] Documentation complete for GitHub Actions provider
- [ ] Migration guide complete for CircleCI → GitHub Actions
- [ ] Examples repo includes GitHub Actions workflows

## Dual-Provider CI: The Ultimate Test

### Running CI on Both CircleCI AND GitHub Actions

To prove cigen's "write once, run anywhere" philosophy, **cigen will run its own CI on BOTH providers simultaneously** from the same config.

**Setup:**

```yaml
# .cigen/config.yml
# Single config that works for BOTH providers

architectures: [x64]

packages:
  rust:
    components: [rustfmt, clippy]
  node:
    package_manager: pnpm

cache:
  - cargo
  - node_modules
  - turbo_cache

source_file_groups:
  rust: ['src/**/*.rs', 'Cargo.*']
  config: ['.cigen/**/*', 'turbo.json', 'package.json']

# Provider-specific settings
circleci:
  dynamic: true
  compile_cigen: true # Compile cigen in setup job for continuation

workflows:
  ci:
    jobs:
      # Same jobs for both providers
      format: { source_files: '@rust' }
      lint: { source_files: '@rust' }
      test: { source_files: ['@rust', '@config'] }
      build: { source_files: '@rust' }
      integration: { source_files: '@rust' }
```

**Generation:**

```bash
# Generate both from same config
cigen generate --provider circleci --output .circleci/config.yml
cigen generate --provider github-actions --output .github/workflows/ci.yml

# Or with multi-provider support (future):
cigen generate --all-providers
```

**Key Differences:**

| Aspect                 | CircleCI                               | GitHub Actions                            |
| ---------------------- | -------------------------------------- | ----------------------------------------- |
| **Workflow structure** | Setup job + dynamic continuation       | Static workflow file                      |
| **Compile cigen**      | ✅ Yes (in setup job for continuation) | ❌ No (workflow is static, pre-generated) |
| **Job skipping**       | Full skip (jobs never start)           | Early exit (~7-38 sec overhead)           |
| **OR dependencies**    | Shim jobs via API                      | Native `\|\|` conditions                  |
| **Matrix builds**      | Separate jobs with suffixes            | Native `strategy.matrix`                  |

**Validation Job (GitHub Actions only):**

Since GitHub Actions workflows are static, add an optional validation job that ensures committed workflows are up-to-date:

```yaml
jobs:
  validate_config:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # Install cigen from source
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Install cigen
        run: cargo install --path .

      # Regenerate and check for drift
      - name: Generate workflow
        run: cigen generate --provider github-actions

      - name: Check for config drift
        run: |
          if ! git diff --quiet .github/workflows/; then
            echo "❌ ERROR: Generated workflow differs from committed version"
            echo "Run 'cigen generate --provider github-actions' and commit changes"
            git diff .github/workflows/
            exit 1
          fi
          echo "✅ Workflow is up-to-date"
```

**Benefits:**

1. ✅ **Validation** - If one provider's workflow breaks, we know immediately
2. ✅ **Redundancy** - CI keeps running even if one provider has an outage
3. ✅ **Feature Testing** - Compare how features work across providers
4. ✅ **Migration Path** - Users can test both before fully switching
5. ✅ **Dogfooding** - We use cigen exactly as users would for multi-provider setups
6. ✅ **Proof of Portability** - Same config, identical test results on both platforms

**What This Tests:**

- ✅ Same config generates valid workflows for both providers
- ✅ Job dependencies work correctly on both
- ✅ Caching works on both (same cache key format)
- ✅ Job skipping works on both (CircleCI: full skip, GitHub: early exit)
- ✅ Package installation works on both
- ✅ Test results are identical across platforms
- ✅ Platform-specific optimizations are applied correctly

## Conclusion

This plan provides a comprehensive roadmap for:

1. **Full GitHub Actions support** in cigen with feature parity to CircleCI
2. **Job skipping** using early-exit pattern (platform limitation workaround)
3. **Self-hosted runners** with pluggable cache backends
4. **Automatic package management** for Rust, Node.js, and more
5. **Self-hosting** - using cigen to generate its own CI **on both CircleCI and GitHub Actions**
6. **Dual-provider CI** - proving "write once, run anywhere" with the same config on both platforms

The implementation follows cigen's philosophy: "If a CI platform doesn't support a feature we need, we build it ourselves." Where GitHub Actions lacks features (dynamic workflow generation), we implement best-effort workarounds (early-exit skip checks). Where it has advantages (native OR dependencies, matrices), we use them fully.

The result: **Write once, run anywhere** - the same cigen config works on CircleCI, GitHub Actions, and future providers, with platform-specific optimizations automatically applied. By running cigen's own CI on both platforms simultaneously, we prove this philosophy works in production.
