# cigen.yml Schema Documentation

This document formally specifies the `cigen.yml` configuration format.

## File Location

CIGen looks for configuration in this order:

1. `cigen.yml` (root of repository)
2. `.cigen/cigen.yml` (hidden directory)
3. Path specified via `--config` flag

## Top-Level Structure

```yaml
# Project metadata (optional)
project:
  name: string
  type: turborepo | default # Monorepo type detection

# Provider configuration (optional)
providers:
  - github
  - circleci
  - buildkite

# Global packages (optional)
packages:
  - string

# Job definitions (required)
jobs:
  <job_id>:
    # Job configuration

# Cache definitions (optional, overrides defaults)
caches:
  <cache_id>:
    # Cache configuration

# Runner definitions (optional)
runners:
  <runner_id>:
    # Runner configuration

# Provider-specific overrides (optional)
provider_config:
  <provider_name>:
    # Provider-specific settings
```

## Job Definition

```yaml
jobs:
  <job_id>:
    # Job dependencies
    needs:
      - <job_id>

    # Matrix build dimensions
    matrix:
      <dimension>:
        - <value1>
        - <value2>

    # Package managers to use
    packages:
      - ruby
      - node
      - docker
      - ...

    # Service containers
    services:
      - postgres:15
      - redis:7
      - mysql:8

    # Environment variables
    env:
      KEY: value

    # Job steps
    steps:
      - run: <command>
      - uses: <module>@<version>
        with:
          <param>: <value>

    # Skip conditions
    skip_if:
      paths_unmodified:
        - <glob_pattern>
      env:
        - <env_var>
      branch:
        - <pattern>

    # Trigger conditions (optional)
    trigger: manual | scheduled
    # or
    trigger:
      tags: <pattern>
      branches: <pattern>

    # Runner class (optional)
    runner: default | large | xlarge | <custom>

    # Artifacts (optional)
    artifacts:
      - path: <glob>
        retention: <duration>
```

## Field Reference

### Project

```yaml
project:
  # Human-readable project name
  name: string

  # Project type for special handling
  type: turborepo | default

  # Default runner for all jobs (optional)
  default_runner: string
```

### Providers

```yaml
providers:
  # List of CI providers to generate configs for
  - github # GitHub Actions
  - circleci # CircleCI
  - buildkite # Buildkite
```

Default: Generate for all providers if omitted.

### Packages

Specify which package managers/tools to use. CIGen automatically:

- Detects versions
- Generates install steps
- Configures caching
- Sets up environment

```yaml
packages:
  - ruby # Detects from .ruby-version, Gemfile
  - node # Detects from .nvmrc, package.json
  - python # Detects from .python-version, requirements.txt
  - go # Detects from go.mod
  - rust # Detects from Cargo.toml
  - docker # Enables Docker daemon
  - terraform # Installs Terraform CLI
```

### Jobs

#### Job ID

The job identifier. Use descriptive names:

- `test` - Test suite
- `lint` - Code linting
- `build` - Build artifacts
- `deploy` - Deployment
- `e2e` - End-to-end tests

Job IDs must be valid YAML keys (alphanumeric + hyphens/underscores).

#### needs

```yaml
needs:
  - setup
  - test
```

Job dependencies. Creates a directed acyclic graph (DAG).

- Jobs run in parallel if no dependencies
- Jobs wait for dependencies to complete
- Circular dependencies are rejected

#### matrix

```yaml
matrix:
  ruby:
    - '3.2'
    - '3.3'
  arch:
    - amd64
    - arm64
```

Generates jobs for all combinations:

- `test-ruby3.2-amd64`
- `test-ruby3.2-arm64`
- `test-ruby3.3-amd64`
- `test-ruby3.3-arm64`

Matrix variables available as `{{ ruby }}`, `{{ arch }}` in steps.

#### packages

```yaml
packages:
  - ruby
  - node
```

Per-job package requirements. Overrides global `packages`.

Auto-generates:

- Install steps (`bundle install`, `npm ci`)
- Caching (gems, node_modules)
- Version setup

#### services

```yaml
services:
  - postgres:15
  - redis:7
  - mysql:8.0
```

Service containers (Docker Compose-style).

Format: `<image>:<tag>`

Provider-specific translation:

- **GitHub Actions**: `services:` block
- **CircleCI**: Multi-image `docker:` array
- **Buildkite**: Docker Compose plugin

#### env

```yaml
env:
  DATABASE_URL: postgres://localhost/test
  REDIS_URL: redis://localhost:6379
  NODE_ENV: test
```

Environment variables available to all steps.

Supports interpolation:

- `{{ matrix.ruby }}` - Matrix variable
- `{{ env.HOME }}` - System environment
- `{{ secrets.API_KEY }}` - Provider secrets

#### steps

Array of commands to execute.

##### Run Step

```yaml
- run: bundle exec rspec
```

Simple command execution.

Multi-line:

```yaml
- run: |
    bundle exec rake db:schema:load
    bundle exec rspec
```

With name:

```yaml
- run:
    name: Run tests
    command: bundle exec rspec
```

##### Uses Step (Module)

```yaml
- uses: docker/build@>=1.1
  with:
    context: .
    push: false
    tags: myapp:latest
```

Reusable modules (like GitHub Actions).

Version constraints:

- `@1.0` - Exact version
- `@>=1.1` - Minimum version
- `@~1.2` - Patch updates only
- `@^1.0` - Minor updates allowed

##### Cache Steps (Manual)

Usually automatic, but can be explicit:

```yaml
- restore_cache:
    keys:
      - gems-{{ checksum "Gemfile.lock" }}

- run: bundle install

- save_cache:
    key: gems-{{ checksum "Gemfile.lock" }}
    paths:
      - vendor/bundle
```

#### skip_if

```yaml
skip_if:
  # Skip if files unchanged
  paths_unmodified:
    - app/**
    - spec/**
    - '!app/assets/**' # Negation

  # Skip if env var set
  env:
    - SKIP_TESTS

  # Skip on branches
  branch:
    - dependabot/*
    - renovate/*
```

Job skipping logic. If conditions match, job is skipped entirely.

**How it works**:

1. Compute work signature (hash of inputs)
2. Compare to last successful run
3. Skip if signature matches

**Default patterns** (if omitted):

- Test jobs: `app/**`, `lib/**`, `spec/**`, `test/**`
- Build jobs: `app/**`, `lib/**`, `Dockerfile`
- Lint jobs: `**/*.rb`, `**/*.js` (language-specific)

#### trigger

```yaml
# Manual trigger (workflow_dispatch)
trigger: manual

# Scheduled (cron)
trigger: scheduled

# Tag-based
trigger:
  tags: v*

# Branch-based
trigger:
  branches:
    - main
    - release/*
```

Job trigger conditions.

Default: Run on all pushes and pull requests.

#### runner

```yaml
runner: large
```

Runner class selection. Maps to provider-specific resources:

- `default` - Standard runner
- `small` - Minimal resources
- `medium` - Standard resources
- `large` - More CPU/RAM
- `xlarge` - Maximum resources

Custom runners defined in `runners:` section.

#### artifacts

```yaml
artifacts:
  - path: dist/**
    retention: 7d
  - path: coverage/**
    retention: 30d
```

Artifacts to store and make available to subsequent jobs.

Provider translation:

- **GitHub Actions**: `actions/upload-artifact@v4`
- **CircleCI**: `store_artifacts`
- **Buildkite**: Artifact upload plugin

### Caches

```yaml
caches:
  bundler:
    paths:
      - vendor/bundle
    key_parts:
      - Gemfile.lock
      - ruby:{{ ruby_version }}
    backend: native | redis | s3

  npm:
    paths:
      - node_modules
      - .npm
    key_parts:
      - package-lock.json
```

Custom cache definitions (overrides automatic caching).

#### key_parts

Array of components for cache key:

- File paths → `{{ checksum "path" }}`
- Variables → `{{ ruby_version }}`
- Literals → `v1`

Final key: `gems-sha256(Gemfile.lock)-ruby3.3-v1`

#### backend

- `native` - Provider's built-in caching
- `redis` - Redis-based cache (requires config)
- `s3` - S3-compatible object storage

### Runners

```yaml
runners:
  default:
    provider_config:
      github:
        runs_on: ubuntu-latest
      circleci:
        resource_class: medium

  large:
    provider_config:
      github:
        runs_on: ubuntu-latest-8-cores
      circleci:
        resource_class: xlarge
      buildkite:
        agents:
          queue: high-cpu
```

Runner class definitions with provider-specific configuration.

### Provider Config

```yaml
provider_config:
  github:
    # GitHub Actions specific
    workflows:
      ci:
        permissions:
          contents: read
          pull-requests: write

  circleci:
    # CircleCI specific
    orbs:
      - slack: circleci/slack@4.12
    version: 2.1

  buildkite:
    # Buildkite specific
    agents:
      queue: default
```

Provider-specific overrides and settings.

## Variable Interpolation

Variables available in strings:

### Matrix Variables

```yaml
matrix:
  ruby:
    - '3.3'

steps:
  - run: echo "Testing with Ruby {{ matrix.ruby }}"
```

### Environment Variables

```yaml
env:
  APP_NAME: myapp

steps:
  - run: echo "Building {{ env.APP_NAME }}"
```

### Built-in Variables

```yaml
steps:
  - run: |
      echo "Commit: {{ git.sha }}"
      echo "Branch: {{ git.branch }}"
      echo "Tag: {{ git.tag }}"
      echo "Runner: {{ runner.os }}"
```

### Functions

```yaml
- run: bundle --version > {{ checksum "Gemfile.lock" }}
```

Available functions:

- `{{ checksum "path" }}` - SHA256 of file
- `{{ env.VAR }}` - Environment variable
- `{{ secrets.KEY }}` - Provider secret

## Conventions

### Automatic Behavior

CIGen applies conventions to reduce config:

1. **Checkout**: Always first step (auto-added)
2. **Caching**: Automatic for declared packages
3. **Version detection**: From lock files, version files
4. **Skip logic**: Smart defaults based on job name
5. **Workflow names**: Derived from file/job structure

### Job Name Conventions

Job names trigger smart defaults:

- `test`, `spec`, `rspec` → Test skip patterns
- `lint`, `rubocop`, `eslint` → Lint skip patterns
- `build`, `compile` → Build skip patterns
- `deploy` → Manual trigger by default

### Package Conventions

Package managers trigger automatic steps:

- `ruby` → `bundle install`, cache `vendor/bundle`
- `node` → `npm ci`, cache `node_modules`
- `python` → `pip install -r requirements.txt`, cache `.venv`

## Validation

Run `cigen validate` to check:

- ✅ YAML syntax
- ✅ Schema compliance
- ✅ No circular dependencies
- ✅ Valid job references
- ✅ Provider compatibility
- ✅ Module version constraints

## Example: Full Config

```yaml
project:
  name: myapp

providers:
  - github
  - circleci

jobs:
  setup:
    packages:
      - ruby
      - node
    steps:
      - run: bundle install --deployment
      - run: npm ci

  test:
    needs:
      - setup
    matrix:
      ruby:
        - '3.2'
        - '3.3'
    packages:
      - ruby
    services:
      - postgres:15
    env:
      DATABASE_URL: postgres://postgres@localhost/test
    steps:
      - run: bundle exec rspec
    skip_if:
      paths_unmodified:
        - app/**
        - spec/**

  build:
    needs:
      - test
    packages:
      - docker
    steps:
      - uses: docker/build@1.0
        with:
          push: false

caches:
  bundler:
    paths:
      - vendor/bundle
    key_parts:
      - Gemfile.lock
      - ruby:{{ ruby_version }}
```

This generates complete CI configs for GitHub Actions and CircleCI with:

- 8 total jobs (1 setup + 2×3 matrix test + 1 build)
- Automatic caching
- Service containers
- Skip logic
- Docker build

All from ~40 lines of config.
