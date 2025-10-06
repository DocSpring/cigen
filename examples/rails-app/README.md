# Rails Application Example

This demonstrates a production Rails application similar to DocSpring with:

- Multiple languages (Ruby + Node.js)
- Database and cache services (PostgreSQL + Redis)
- Matrix builds (Ruby versions × architectures)
- Parallel jobs (test + lint)
- Docker builds with layer caching
- Deployment workflows
- Smart skip logic

## Key Features Demonstrated

### 1. Job Dependencies

```yaml
jobs:
  test:
    needs:
      - setup # Waits for setup to complete
```

Creates a DAG: `setup` → `test` + `lint` → `build` → `deploy`

### 2. Matrix Builds

```yaml
matrix:
  ruby:
    - '3.2'
    - '3.3'
  arch:
    - amd64
    - arm64
```

Generates 4 jobs: `test-ruby3.2-amd64`, `test-ruby3.2-arm64`, `test-ruby3.3-amd64`, `test-ruby3.3-arm64`

### 3. Services (Docker Compose-style)

```yaml
services:
  - postgres:15
  - redis:7
```

Provider plugins translate to:

- **GitHub Actions**: `services:` blocks
- **CircleCI**: Docker `image:` array
- **Buildkite**: Docker Compose plugin

### 4. Skip Logic (Job Skipping)

```yaml
skip_if:
  paths_unmodified:
    - app/**
    - spec/**
```

If no files in these paths changed since last successful run, skip the job entirely. This is the **killer feature** for monorepos and large codebases.

### 5. Module System

```yaml
steps:
  - uses: docker/build@>=1.1
    with:
      push: false
      cache-from: type=registry,ref=myapp:latest
```

Uses a reusable module (like GitHub Actions) with semantic versioning.

### 6. Deployment Workflows

```yaml
deploy-staging:
  trigger: manual # Workflow dispatch

deploy-production:
  trigger:
    tags: v* # Only on git tags
```

Provider-agnostic deployment triggers.

## What Gets Auto-Generated

### GitHub Actions Output

`.github/workflows/ci.yml`:

- Workflow with all jobs
- Matrix strategy for test job
- Service containers for postgres/redis
- Caching for gems and node_modules
- Skip logic using path filters

`.github/workflows/deploy-staging.yml`:

- Manual workflow_dispatch trigger

`.github/workflows/deploy-production.yml`:

- Tag filter (on: push: tags: v\*)

### CircleCI Output

`.circleci/config.yml`:

- Workflows with job dependencies
- Docker executor with multi-image support
- CircleCI native caching
- Dynamic config for path-based skipping

### Buildkite Output

`.buildkite/pipeline.yml`:

- Pipeline with wait steps
- Docker Compose plugin for services
- Cache plugin
- Conditional step execution

## Comparison to Manual Config

### Without CIGen (GitHub Actions)

You'd write ~300 lines of YAML with:

- Repetitive cache configurations
- Manual service container setup
- No skip logic (or complex custom scripts)
- Copy-paste matrix definitions
- Provider-specific syntax everywhere

### With CIGen

One 100-line `cigen.yml` that:

- Works for GitHub, CircleCI, Buildkite
- Auto-configures caching optimally
- Generates skip logic automatically
- Uses conventions to reduce boilerplate

## Estimated Time Savings

**Setup**: 2 hours → 10 minutes
**Maintenance**: 1 hour/month → 5 minutes/month
**Provider migration**: 2 weeks → 1 command (`cigen render --provider buildkite`)

## Usage

```bash
# Plan changes (shows what will be generated)
cigen plan

# Generate all provider configs
cigen render

# Generate specific provider
cigen render --provider github
cigen render --provider circleci

# Validate config
cigen validate

# Explain why a job will/won't run
cigen explain job test
```
