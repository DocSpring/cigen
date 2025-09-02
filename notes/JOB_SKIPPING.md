# Job Skipping (Skip Cache)

## Overview

Cigen's job skipping system automatically skips jobs when their source files haven't changed since the last successful run. This is the core efficiency feature that eliminates unnecessary work in CI pipelines while maintaining correctness.

## How It Works

When a job defines `source_files`, Cigen automatically injects skip cache logic:

1. **Pre-execution**: Calculate SHA-256 hash of all source files
2. **Skip check**: If a cache marker exists for this hash, skip the job entirely using `circleci step halt`
3. **Post-execution**: On successful completion, record the hash in the skip cache

## Source File Tracking

### Named Groups

Define reusable source file groups in `config/source_file_groups.yml`:

```yaml
source_file_groups:
  ruby:
    - '**/*.rb'
    - '**/*.rake'
    - 'Gemfile*'
    - 'Rakefile'
    - '.ruby-version'

  javascript:
    - 'client/**/*'
    - 'package.json'
    - 'pnpm-lock.yaml'
    - 'config/webpack/**/*'
    - 'babel.config.js'
```

Use with the `@` prefix to reference named groups:

```yaml
# workflows/test/jobs/ruby_lint.yml
source_files: '@ruby' # References the ruby group
```

### Inline Source Files

For job-specific source files, define them inline:

```yaml
# workflows/test/jobs/shellcheck.yml
source_files:
  - 'scripts/**/*'
  - '*.sh'
```

### Mixed References

Combine named groups and custom globs:

```yaml
source_files:
  - '@ruby' # Named group
  - '@javascript' # Another named group
  - 'config/custom/**/*' # Custom glob
  - 'Dockerfile' # Single file
```

## Automatic Template Injection

Cigen automatically includes relevant CI template files in source file tracking:

- When processing `workflows/test/jobs/ruby_lint.yml`, Cigen automatically includes `.circleci/src/ci_jobs/ruby_lint.yml.erb` in the source file hash
- When the job template changes, the source file hash changes, busting the skip cache
- You should **NOT** manually include these template files in your source_file_groups

This ensures that changes to the CI job definitions themselves trigger re-runs of affected jobs.

## Architecture Awareness

Job skipping is architecture-aware for multi-arch builds:

```yaml
# This job runs on both amd64 and arm64
architectures: ['amd64', 'arm64']
source_files: '@ruby'
```

Generates separate skip cache entries:

- `/tmp/cigen_skip_cache/job_{hash}_amd64`
- `/tmp/cigen_skip_cache/job_{hash}_arm64`

Each architecture can be skipped independently based on its own completion status.

## Generated Skip Logic

For a job with `source_files`, Cigen injects these steps:

### 1. Source File Hash Calculation

```yaml
- run:
    name: Calculate source file hash
    command: |
      echo "Calculating hash for source files..."
      TEMP_HASH_FILE="/tmp/source_files_for_hash"
      rm -f "$TEMP_HASH_FILE"

      # Add each source file pattern
      [ -f **/*.rb ] && echo **/*.rb || true >> "$TEMP_HASH_FILE"
      [ -f Gemfile* ] && echo Gemfile* || true >> "$TEMP_HASH_FILE"
      # ... (for each source file pattern)

      if [ -f "$TEMP_HASH_FILE" ]; then
          export JOB_HASH=$(sort "$TEMP_HASH_FILE" | xargs sha256sum | sha256sum | cut -d' ' -f1)
          echo "Hash calculated: $JOB_HASH"
      else
          export JOB_HASH="empty"
          echo "No source files found, using empty hash"
      fi
```

### 2. Skip Check

```yaml
- run:
    name: Check if job should be skipped
    command: |
      if [ -f "/tmp/cigen_skip_cache/job_${JOB_HASH}_amd64" ]; then
          echo "Job already completed successfully for this file hash. Skipping..."
          circleci step halt
      else
          echo "No previous successful run found. Proceeding with job..."
          mkdir -p /tmp/cigen_skip_cache
      fi
```

### 3. Completion Recording

```yaml
- run:
    name: Record job completion
    command: |
      echo "Recording successful completion for hash ${JOB_HASH}"
      echo "$(date): Job completed successfully" > "/tmp/cigen_skip_cache/job_${JOB_HASH}_amd64"
```

## Benefits

1. **Massive Time Savings**: Skip jobs when source files haven't changed
2. **Resource Efficiency**: Reduce CI costs and resource usage
3. **Faster Feedback**: Get results faster for unchanged components
4. **Automatic**: No manual intervention required
5. **Safe**: Only skips on successful completion, never on failures

## Use Cases

Perfect for:

- **Linting jobs**: Skip if source code hasn't changed
- **Test suites**: Skip if relevant code hasn't changed
- **Build jobs**: Skip if source files and configs haven't changed
- **Security scans**: Skip if code hasn't changed

## Best Practices

1. **Be specific with source files**: Only include files that actually affect the job
2. **Include config files**: Add relevant config files to source_files
3. **Use named groups**: For reusable source file patterns
4. **Don't over-specify**: Avoid including files that don't affect job outcome
5. **Let Cigen handle templates**: Don't manually include CI template files

## Examples

### Ruby Linting Job

```yaml
# workflows/test/jobs/ruby_lint.yml
image: cimg/ruby:3.3.5
source_files: '@ruby'

steps:
  - run:
      name: RuboCop
      command: bundle exec rubocop
```

### JavaScript Testing Job

```yaml
# workflows/test/jobs/js_test.yml
image: cimg/node:18
source_files:
  - '@javascript'
  - 'jest.config.js'
  - 'tsconfig.json'

steps:
  - run:
      name: Jest Tests
      command: npm test
```

### Custom Build Job

```yaml
# workflows/build/jobs/docker_build.yml
image: cimg/base:stable
source_files:
  - 'Dockerfile*'
  - 'src/**/*'
  - 'package.json'
  - '.dockerignore'

steps:
  - run:
      name: Build Docker Image
      command: docker build -t myapp .
```

## Troubleshooting

### Job Always Runs

If a job with `source_files` never gets skipped:

1. Check that source file patterns are correct
2. Verify files exist and patterns match
3. Look for broad patterns that always change (e.g., `**/*`)
4. Check if CI template files are changing between runs

### Job Never Runs

If a job gets skipped when it shouldn't:

1. Check if source files are too narrow
2. Add missing config files to source_files
3. Clear skip cache if needed: `rm -rf /tmp/cigen_skip_cache`

### Architecture Issues

For multi-arch builds:

1. Each architecture maintains separate skip cache
2. Both can skip independently based on their completion status
3. If one arch fails, only that arch will re-run next time
