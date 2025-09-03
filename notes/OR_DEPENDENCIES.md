# OR Dependencies Implementation for CircleCI

## Problem

CircleCI only supports AND dependencies (all requirements must complete). We need OR dependencies (any requirement can trigger).

## Solution: Automatic Shim Job Generation

### 1. OR Dependencies

When a job specifies OR dependencies:

```yaml
# In cigen job definition:
job:
  name: build_app_image
  requires_any:
    - approve_build_app_image # Manual approval
    - deploy_staging # Auto-trigger from deployment
    - deploy_eu
    - deploy_us
```

Cigen should automatically generate:

- The main approval job (`approve_build_app_image`)
- Shim jobs that programmatically approve it:
  - `approve_build_app_image_from_staging`
  - `approve_build_app_image_from_eu`
  - `approve_build_app_image_from_us`

Each shim job:

1. Requires the respective trigger job
2. Uses CircleCI API to approve the main approval job
3. Has the built-in `automated_approval` command

### 2. GitHub Status Patch Job

Automatically inject `patch_approval_jobs_status` job that:

- Runs after all test jobs complete
- Sets GitHub status to success for approval jobs
- Fixes CircleCI's pending status issue

This should be:

- Enabled by default for CircleCI + GitHub
- Configurable via `fix_github_status: false` to opt-out

## Implementation Plan

### Config Schema Addition

```yaml
# In job definition
requires_any: [job1, job2, job3] # New field

# In global config
circleci:
  fix_github_status: true # Default true
```

### Built-in Commands

Create `automated_approval` command in CircleCI provider:

```yaml
automated_approval:
  parameters:
    job_name:
      type: string
  steps:
    - run:
        name: Auto-approve << parameters.job_name >>
        command: |
          # CircleCI API call to approve job
```

### Generator Logic

In CircleCI generator:

1. Detect `requires_any` fields
2. Transform first job to approval type
3. Generate shim jobs for other dependencies
4. Add `patch_approval_jobs_status` if GitHub is detected

## Example Transformation

Input (cigen):

```yaml
build_app_image:
  requires_any:
    - approve_manually
    - deploy_staging
    - deploy_eu
```

Output (CircleCI):

```yaml
# Main approval job
approve_manually:
  type: approval

# Actual job
build_app_image:
  requires:
    - approve_manually

# Auto-generated shim jobs
approve_manually_from_deploy_staging:
  requires:
    - deploy_staging
  steps:
    - automated_approval:
        job_name: approve_manually

approve_manually_from_deploy_eu:
  requires:
    - deploy_eu
  steps:
    - automated_approval:
        job_name: approve_manually
```

## Benefits

- Users just specify intent (`requires_any`)
- Cigen handles platform-specific workarounds
- Cleaner, more maintainable configs
- Portable across CI platforms
