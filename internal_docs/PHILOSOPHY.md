# Cigen Philosophy: Write Once, Run Anywhere

## Core Principle

**If a CI platform doesn't support a feature we need, we build it ourselves.**

Cigen is not just a configuration generator - it's a universal CI abstraction layer that allows you to write your CI configuration once and run it on ANY CI platform, regardless of that platform's limitations or quirks.

## Key Abstractions

### 1. Job Skipping / Caching

**What you write:** "Skip this job if these files haven't changed"

**How cigen implements it:**

- **CircleCI**: Uses setup workflows with dynamic config to completely skip jobs
- **GitHub Actions**: Injects an early-exit step that checks SHA256 hashes and aborts if files haven't changed
- **Other platforms**: Best-effort implementation based on available features

Our approach uses a **top-down SHA256 hash** of actual file contents, which is far more reliable than GitHub's brittle bottom-up `paths` filter that:

- Misses changes in dependencies outside the specified paths
- Prevents re-runs when external issues are fixed
- Can't handle intermittent failures that need retries

### 2. OR Dependencies

**What you write:** "Run this job when approval_A OR approval_B happens"

**How cigen implements it:**

- **CircleCI**: Automatically generates "shim" jobs that use the CircleCI API to programmatically approve jobs, converting OR logic into AND logic that CircleCI understands
- **GitHub Actions**: Could use workflow_dispatch events or other mechanisms
- **Other platforms**: Platform-specific workarounds

Example: In DocSpring's workflow, `build_app_image` should run when:

- Someone manually approves `approve_build_app_image`, OR
- Someone approves any deployment (staging/EU/US)

Since CircleCI only supports AND dependencies, cigen automatically creates `approve_build_app_image_staging/eu/us` shim jobs that programmatically approve the main approval job via API.

### 3. Dynamic Architecture Matrix

**What you write:** "Run this job on architectures: [amd64, arm64]"

**How cigen implements it:**

- **CircleCI**: Generates separate jobs with architecture suffixes
- **GitHub Actions**: Uses matrix strategies
- **Other platforms**: Best available approach

## Implementation Strategy

### Built-in Commands

Cigen should provide built-in commands for common workarounds:

- `automated_approval`: Programmatically approves other jobs via API
- `skip_if_unchanged`: Checks file hashes and exits early
- `wait_for_any`: Implements OR dependencies
- `matrix_job`: Handles multi-architecture/multi-version builds

### Platform Detection

Cigen detects platform capabilities and automatically:

1. Uses native features when available
2. Implements workarounds when features are missing
3. Warns users about limitations that can't be worked around

## Benefits

1. **Portability**: Move between CI platforms without rewriting configs
2. **Consistency**: Same behavior across different platforms
3. **Simplicity**: Write what you want, not how to achieve it
4. **Evolution**: As platforms add features, cigen can switch from workarounds to native implementations

## Example Transformations

### Job Caching

```yaml
# What you write in cigen:
job:
  name: test
  source_files: '@ruby'  # Only run if Ruby files changed
  steps: [...]

# What cigen generates for CircleCI:
# - Uses setup workflow to check hashes
# - Skips entire job if unchanged

# What cigen generates for GitHub:
job:
  steps:
    - name: Check if files changed
      run: |
        if cache_hit "${SHA256_HASH}"; then
          echo "Files unchanged, skipping job"
          exit 0
        fi
    - ... actual steps ...
```

### OR Dependencies

```yaml
# What you write in cigen:
job:
  requires_any: ['approval_A', 'approval_B']

# What cigen generates for CircleCI:
# Creates automated shim jobs that convert OR to AND
approval_A_shim:
  requires: ['approval_A']
  steps:
    - automated_approval: 'target_job'

approval_B_shim:
  requires: ['approval_B']
  steps:
    - automated_approval: 'target_job'
```

## Future Vision

As cigen evolves, it should:

1. Build a library of platform-specific workarounds
2. Share workarounds across the community
3. Influence CI platforms to add missing features natively
4. Provide performance metrics comparing native vs workaround implementations

The ultimate goal: **You describe WHAT you want your CI to do, and cigen figures out HOW to make it happen on any platform.**
