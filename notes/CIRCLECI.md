# CircleCI Provider Documentation

## Overview

The CircleCI provider generates CircleCI configuration files from cigen's universal format. It includes several CircleCI-specific features and workarounds to address platform limitations.

## Configuration

### Basic Configuration

```yaml
provider: circleci
output_path: .circleci
output_filename: config.yml

circleci:
  fix_github_status: true # Auto-inject GitHub status patch job
```

### GitHub Status Fix

CircleCI has a limitation where approval jobs don't properly update GitHub commit statuses, causing GitHub branch protection rules to fail even when approvals are granted.

When `circleci.fix_github_status: true` is set, cigen automatically injects a `patch_approval_jobs_status` job that:

1. Waits for all test jobs to complete
2. Uses the GitHub API to manually set approval job statuses to "success"
3. Allows GitHub branch protection rules to pass

**Required Environment Variable:**

- `GITHUB_PERSONAL_ACCESS_TOKEN` - GitHub personal access token with `repo:status` permissions

**Setup Instructions:**

1. Create a GitHub personal access token with `repo:status` scope
2. Add it to your CircleCI project as an environment variable named `GITHUB_PERSONAL_ACCESS_TOKEN`
3. Enable the feature in your `cigen.yml` configuration

The patch job is automatically injected when:

- `circleci.fix_github_status: true` is configured
- The workflow contains at least one approval job (`type: approval`)

## Job Types

### Approval Jobs

```yaml
jobs:
  approve_deploy:
    type: approval
    # No other configuration needed
```

Approval jobs are converted to CircleCI's `type: approval` jobs.

### Multi-Architecture Jobs

```yaml
jobs:
  build_image:
    architectures: [amd64, arm64]
    # ... other job config
```

Creates separate jobs for each architecture:

- `build_image_amd64`
- `build_image_arm64`

If only one architecture is specified, no suffix is added.

## OR Dependencies Workaround

CircleCI doesn't natively support OR dependencies (job A requires either job B OR job C). cigen implements this using automated approval shim jobs:

```yaml
jobs:
  deploy_job:
    requires_any: [manual_approval, automated_trigger]
```

This creates:

1. The original `deploy_job`
2. Automated approval shim jobs that approve `deploy_job` when `automated_trigger` completes
3. Proper dependency chains for both paths

## Context Support

```yaml
jobs:
  deploy_production:
    context:
      - deploy-production
      - aws-credentials
```

Maps directly to CircleCI contexts for environment-specific secrets and variables.

## Dynamic Configuration

```yaml
setup: true
# or
dynamic: true

parameters:
  run_tests:
    type: boolean
    default: false
```

Enables CircleCI's dynamic configuration features for conditional workflow execution.

## Template Commands

cigen includes built-in template commands for common CircleCI patterns:

- `checkout` - Standard git checkout
- `restore_cache` - Cache restoration with automatic key generation
- `save_cache` - Cache saving with automatic key generation
- `automated_approval` - API-based job approval for OR dependency workarounds

## Cache Integration

When using the automatic cache injection system (see `CACHING.md`), cigen:

1. Generates SHA-256 checksums for cache keys
2. Injects appropriate `restore_cache` and `save_cache` steps
3. Handles cache dependencies across jobs automatically

## Validation

The generator automatically validates generated configurations using the CircleCI CLI:

```bash
circleci config validate .circleci/config.yml
```

This ensures the generated configuration is syntactically correct and follows CircleCI best practices.

## Limitations & Workarounds

### GitHub Status Updates

- **Problem**: Approval jobs don't update GitHub commit statuses
- **Solution**: Auto-injected `patch_approval_jobs_status` job with GitHub API calls

### OR Dependencies

- **Problem**: No native support for "requires any of" dependencies
- **Solution**: Automated approval shim jobs using CircleCI API

### Multi-Architecture Builds

- **Problem**: No built-in matrix builds
- **Solution**: Generate separate jobs with architecture suffixes

## Environment Variables

Required for various features:

- `GITHUB_PERSONAL_ACCESS_TOKEN` - For GitHub status fix feature
- `CIRCLE_TOKEN` - For automated approval API calls (OR dependencies)
- Standard CircleCI variables (`CIRCLE_SHA1`, `CIRCLE_BUILD_URL`, etc.) - Automatically available

## Best Practices

1. **Always enable GitHub status fix** when using approval jobs with GitHub branch protection
2. **Use contexts** for environment-specific secrets rather than project-level environment variables
3. **Test configurations locally** using `circleci config validate` before committing
4. **Use multi-architecture builds** for Docker images that need to support multiple platforms
5. **Leverage dynamic configuration** for complex conditional workflows
