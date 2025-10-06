# Monorepo Example

This demonstrates how CIGen handles monorepos with selective job execution based on affected projects.

## The Problem

In a monorepo with 50 apps/libs:

- **Without optimization**: Every PR runs all 50 test suites (30+ minutes)
- **With CIGen skip logic**: Only run tests for affected projects (2-5 minutes)

This is a **10x speedup** for typical PRs that touch 1-2 projects.

## How It Works

### 1. Nx Integration

CIGen reads your Nx configuration:

```json
// nx.json
{
  "affected": {
    "defaultBase": "main"
  }
}
```

```json
// apps/api/project.json
{
  "name": "api",
  "targets": {
    "test": { "executor": "@nx/jest:jest" },
    "build": { "executor": "@nx/webpack:webpack" }
  }
}
```

### 2. Affected Detection

CIGen automatically computes affected projects:

```bash
# PR changes libs/shared/utils
$ cigen plan

Detecting affected projects...
  ✓ libs/shared/utils (modified)
  ✓ apps/api (depends on shared/utils)
  ✓ apps/web (depends on shared/utils)

Skipping unaffected projects...
  ⊘ apps/admin (no dependency)
  ⊘ apps/mobile (no dependency)
  ⊘ libs/auth (no dependency)

Jobs to run:
  + lint (3 projects)
  + test (3 projects × 2 node versions = 6 jobs)
  + build (3 projects)
  + e2e (2 apps × 2 browsers = 4 jobs)

Total: 16 jobs instead of 100+ jobs
```

### 3. Generated Skip Logic

**GitHub Actions** output:

```yaml
jobs:
  test:
    # Skip if no source files changed in affected projects
    if: |
      github.event_name == 'push' ||
      (contains(github.event.pull_request.files, 'apps/api/') ||
       contains(github.event.pull_request.files, 'apps/web/') ||
       contains(github.event.pull_request.files, 'libs/shared/utils/'))
```

**CircleCI** output uses dynamic config to only generate jobs for affected projects.

## Matrix Builds in Monorepos

```yaml
test:
  matrix:
    node:
      - '18'
      - '20'
```

For 3 affected projects × 2 Node versions = 6 parallel jobs.

If only 1 project is affected × 2 Node versions = 2 jobs (not 6).

## Artifacts

```yaml
build:
  artifacts:
    - path: dist/**
      retention: 7d
```

Stores build outputs for later jobs (e2e tests, deployment).

**Provider translation**:

- **GitHub Actions**: `actions/upload-artifact@v4`
- **CircleCI**: `store_artifacts`
- **Buildkite**: Artifact upload plugin

## The Killer Feature: Work Signatures

Beyond path-based skipping, CIGen can compute **work signatures**:

```
signature = hash(
  source_files +
  dependencies +
  env_vars +
  CI_config
)
```

If signature matches last successful run → skip entire job.

**Example**:

```bash
$ cigen explain job test

Job: test (api)
  Status: SKIP
  Reason: Work signature matches last successful run
  Last run: commit abc123 (2 hours ago)
  Signature: sha256:7f3a8b2c...

  Signature includes:
    - 42 source files in apps/api/src/**
    - 3 dependencies: @nx/jest, jest, typescript
    - 2 env vars: NODE_ENV, DATABASE_URL
    - CI config: test command unchanged
```

## Time Savings

**Before CIGen** (all projects, every PR):

- Lint: 5 min
- Test: 25 min
- Build: 10 min
- E2E: 15 min
- **Total**: 55 minutes

**With CIGen** (1-2 affected projects):

- Lint: 30 sec
- Test: 2 min
- Build: 1 min
- E2E: 2 min
- **Total**: 5.5 minutes

**90% reduction** in CI time for typical PRs.

## Comparison to Turborepo/Nx Remote Cache

| Feature              | Turborepo/Nx | CIGen                |
| -------------------- | ------------ | -------------------- |
| Local caching        | ✅           | ✅ (via tool)        |
| Remote caching       | ✅           | ✅ (work signatures) |
| Affected detection   | ✅           | ✅                   |
| Multi-provider       | ❌           | ✅                   |
| Job skipping in CI   | ❌\*         | ✅                   |
| Path-based filtering | Partial      | ✅                   |

\*Turborepo/Nx run tasks but use cache. CIGen **skips jobs entirely** (faster, cheaper).

## Usage

```bash
# Show what will run
cigen plan

# Explain why a job will/won't run
cigen explain job test

# Generate with affected detection
cigen render

# Force run all (override skip logic)
cigen render --no-skip
```
