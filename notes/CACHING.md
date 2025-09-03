# Cigen Caching System

========================
TODO: switch from: cache: gems, cache: node_modules, etc.
to "packages": gems, node, etc.

### packages is a better name for package management if we're going to handle the install as well.

=======================

## Overview

Cigen provides an intelligent caching system that automatically generates optimal cache keys based on your project's runtime environments and dependencies. The system is designed to be both smart by default and fully customizable.

## Cache Key Structure

All cache keys follow this pattern:

```
{{ os }}-{{ os_version }}-{{ arch }}-{{ cache_name }}-{{ versions }}-{{ checksum }}
```

Where:

- **Platform prefix** (`os`, `os_version`, `arch`) - Automatically included for all caches
- **cache_name** - The name you give to the cache (e.g., `gems`, `node_modules`)
- **versions** - Runtime versions detected from version files (e.g., `ruby3.3.0`)
  - The cache is invalidated and rebuilt from scratch if any versions change.
  - This component is omitted entirely if no versions are specified in the cache definition
- **checksum** - Combined hash of dependency files (e.g. package lock files)
  - If no exact match is found, the most recent version of the cache will be restored with the same name and versions.

## Built-in Cache Types

Cigen includes built-in definitions for common cache types:

### Ruby Gems

```yaml
cache_definitions:
  gems:
    versions:
      - ruby
      - bundler
    checksum_sources:
      - Gemfile
      - Gemfile.lock
    paths:
      - vendor/bundle
      - .bundle

version_sources:
  ruby:
    - file: .ruby-version
    - file: .tool-versions
      pattern: 'ruby (.+)'
    - command: "grep -A1 'RUBY VERSION' Gemfile.lock | tail -n1"
      parse_version: false
    - command: 'ruby --version'
  bundler:
    - command: "grep -A1 'BUNDLED WITH' Gemfile.lock | tail -n1 | tr -d ' '"
      parse_version: false
    - command: 'bundler --version'
```

Example cache key:

```
linux-ubuntu22.04-amd64-gems-ruby3.4.5-bundler2.6.3-abc123def456
```

### Node Modules

```yaml
cache_definitions:
  node_modules:
    versions:
      - node
      - detect:
          - npm
          - yarn
          - bun
          - pnpm
    checksum_sources:
      - package.json
      - detect:
          - package-lock.json
          - yarn.lock
          - bun.lockb
          - pnpm-lock.yaml
    paths:
      - node_modules

version_sources:
  node:
    - file: .node-version
    - file: .nvmrc
    - file: .tool-versions
      pattern: 'node (.+)'
    - command: 'node --version'
  npm:
    - command: 'npm --version'
  yarn:
    - command: 'yarn --version'
  bun:
    - command: 'bun --version'
  pnpm:
    - command: 'pnpm --version'
```

Example cache key: `linux-ubuntu22.04-amd64-node_modules-node20.11.0-npm10.2.4-def456abc123`

### Python

```yaml
cache_definitions:
  pip:
    versions:
      - python
      - pip
    checksum_sources:
      - detect:
          - requirements.txt
          - Pipfile
      - detect_optional:
          - requirements.lock
          - Pipfile.lock
    paths:
      - detect:
          - .venv
          - venv
      - ~/.cache/pip

version_sources:
  python:
    - file: .python-version
    - file: .tool-versions
      pattern: 'python (.+)'
    - file: runtime.txt # Heroku-style
    - command: 'python --version'
  pip:
    - command: 'pip --version'
  go:
    - file: .go-version
    - file: go.mod
      pattern: '^go (.+)'
    - command: 'go version'
```

Example cache key: `linux-ubuntu22.04-amd64-pip-python3.12.1-pip24.0-789abcdef012`

## Using Caches in Jobs

### Automatic Cache Injection

When you use the `cache:` configuration in a job, CIGen automatically injects cache restore and save steps.

### Using Built-in Cache Definitions

The simplest way is to use the built-in cache definitions directly:

```yaml
# workflows/test/jobs/install_gems.yml
cache: gems # Uses all defaults from the gems cache definition

steps:
  - name: Install dependencies
    run: bundle install
```

This automatically injects:

1. A `restore_cache` step before your steps
2. A `save_cache` step after your steps

The generated output includes:

- Versions (ruby, bundler) in the cache key
- Checksum sources (Gemfile, Gemfile.lock)
- Default paths (vendor/bundle, .bundle)

### Using Multiple Caches

```yaml
# workflows/test/jobs/install_deps.yml
cache:
  - node_modules # Uses node_modules definition
  - pip # Uses pip definition
```

### Overriding Specific Parts

You can override just the parts you need - the configuration gets merged with the cache definition:

```yaml
# workflows/test/jobs/install_gems.yml
cache:
  gems:
    paths: # Override just the paths
      - vendor/rubygems
      - .bundle
    # versions and checksum_sources remain unchanged from the definition
```

### Creating Custom Caches

You can also define a completely custom cache inline:

```yaml
# workflows/test/jobs/build_assets.yml
cache:
  assets:
    versions:
      - node
    checksum_sources:
      - package.json
      - webpack.config.js
    paths:
      - .webpack-cache
      - public/assets
```

### Reusing Cache Definitions

Use the `type` field to base a custom cache on an existing definition:

```yaml
cache:
  ml_models:
    type: python_ml # Reuse versions and checksum_sources from python_ml
    paths: # But use different paths
      - models/trained
      - data/processed
```

All of these generate cache keys following the same pattern:

```
linux-ubuntu22.04-amd64-gems-ruby3.3.0-bundler2.5.6-abc123def456
```

## Customizing Cache Definitions

### Extending Built-in Caches

You can override or extend the built-in cache definitions in your `config.yml`:

```yaml
# .cigen/config.yml
cache_definitions:
  # Completely override the gems cache
  gems:
    versions:
      - ruby
      - bundler
    checksum_sources:
      - Gemfile
      - Gemfile.lock
      - gems.locked # Custom lockfile
    paths:
      - vendor/ruby # Override default paths
      - .bundle
```

### Adding Custom Cache Types

Define new cache types for your specific needs:

```yaml
# .cigen/config.yml
cache_definitions:
  python_ml:
    versions:
      - python
      - pip
    checksum_sources:
      - requirements.txt
      - requirements-ml.txt
      - models/config.json
    paths:
      - .venv
      - models/cache
      - ~/.cache/torch

  go_modules:
    versions:
      - go
    checksum_sources:
      - go.sum
    paths:
      - ~/go/pkg/mod
      - .cache/go-build

  # Caches without versions - only checksum-based
  assets:
    checksum_sources:
      - package.json
      - webpack.config.js
      - src/**/*.js
      - src/**/*.css
    paths:
      - dist/assets
      - .webpack-cache

  ml_data:
    checksum_sources:
      - data/raw/**/*.csv
      - scripts/process_data.py
    paths:
      - data/processed
      - data/features
```

Example cache keys for version-less caches:

- `linux-ubuntu22.04-amd64-assets-abc123def456`
- `linux-ubuntu22.04-amd64-ml_data-789abcdef012`

### Using Custom Caches

```yaml
# In a job file
cache:
  # Use your custom cache type with all its defaults
  - python_ml

  # Or override specific parts
  python_ml:
    paths:
      - .venv
      - models/pretrained  # Different from default paths

  # Create an ad-hoc cache using an existing definition
  ml_data:
    type: python_ml  # Inherit versions and checksum_sources
    paths:
      - data/processed
      - data/raw
```

## Version Detection

### Version Sources

Version sources are checked in order. The first one found is used:

1. **file** - Read version directly from a file

   ```yaml
   - file: .ruby-version # Contains: 3.3.0
   ```

2. **file with pattern** - Extract version using regex

   ```yaml
   - file: .tool-versions
     pattern: 'ruby (.+)' # Extracts from: ruby 3.3.0
   ```

3. **command** - Run a command to get version

   ```yaml
   - command: 'ruby --version' # Automatically parses version number
   ```

   By default, commands automatically extract version numbers (e.g., `1.2.3`). For special cases:

   ```yaml
   - command: "grep -A1 'BUNDLED WITH' Gemfile.lock | tail -n1 | tr -d ' '"
     parse_version: false # Use the raw output
   ```

### Multiple Versions

If multiple languages are detected, they're concatenated:

```
linux-ubuntu22.04-amd64-myapp-ruby3.3.0-node20.1-abc123def
```

## Cache Paths and Directory Detection

### How `detect` Works with Paths

When defining cache paths, you can use `detect` and `detect_optional` to handle directories that may or may not exist:

```yaml
paths:
  - detect:
      - .venv
      - venv
  - ~/.cache/pip
```

**Important:** Unlike most CI providers that silently continue if a cache path doesn't exist, Cigen adds pre-cache validation:

1. **Regular paths** (e.g., `~/.cache/pip`) - Must exist or the job will fail
2. **`detect` paths** - At least one of the listed paths must exist
3. **`detect_optional` paths** - None need to exist; validation is skipped

This validation happens before cache upload, ensuring your CI configuration is explicit about which paths are required vs optional. This prevents silent failures where expected cache directories are missing.

Example with all three types:

```yaml
paths:
  - node_modules # Required - job fails if missing
  - detect: # At least one must exist
      - .venv
      - venv
      - virtualenv
  - detect_optional: # All are optional
      - .cache/pre-commit
      - ~/.cache/myapp
```

## Checksum Sources

Checksum sources determine when to invalidate the cache:

- **Manifest files** (e.g., `Gemfile`, `package.json`) - Define requested dependencies
- **Lock files** (e.g., `Gemfile.lock`, `package-lock.json`) - Define exact installed versions

Both are included because:

1. Lock files capture all changes from manifest files
2. Including manifest files ensures cache misses if someone changes dependencies without updating the lock file

## Matrix Support

When using matrix builds, cache keys automatically include matrix parameters:

```yaml
# Job with matrix
matrix:
  ruby: ['3.2', '3.3']
  arch: ['amd64', 'arm64']

cache:
  gems:
    - vendor/bundle
```

Generates keys like:

- `linux-ubuntu22.04-amd64-gems-ruby3.2-abc123def`
- `linux-ubuntu22.04-arm64-gems-ruby3.3-abc123def`

## Manual Cache Steps

In addition to automatic cache injection, you can write cache steps manually:

```yaml
steps:
  - restore_cache:
      name: Restore webpack cache
      key: webpack-{{ checksum "webpack.config.js" }}

  - name: Build assets
    run: npm run build

  - save_cache:
      name: Save webpack cache
      key: webpack-{{ checksum "webpack.config.js" }}
      paths:
        - .webpack-cache
```

These manual cache steps:

- Use the same syntax across all CI providers
- Automatically use the configured cache backend
- Are transformed during compilation to use the appropriate backend commands

## Cache Backends

CIGen supports multiple cache storage backends that can be configured globally or per-cache:

```yaml
# config.yml
cache_backends:
  default: native # Use CI provider's native cache

  # Configure backends for different scenarios
  native:
    # No configuration needed - uses provider's built-in cache

  s3:
    bucket: my-cache-bucket
    region: us-east-1
    prefix: cigen-cache/

  redis:
    url: redis://cache.example.com:6379
    ttl: 604800 # 7 days

  minio:
    endpoint: minio.internal:9000
    bucket: ci-cache
    access_key: ${MINIO_ACCESS_KEY}
    secret_key: ${MINIO_SECRET_KEY}

# Per-cache backend override
cache_definitions:
  ml_models:
    backend: s3 # Large models go to S3
    versions:
      - python
    paths:
      - models/

# Per-runner backend selection
runners:
  self_hosted:
    cache_backend: minio # Self-hosted runners use MinIO
  cloud:
    cache_backend: native # Cloud runners use native cache
```

When CIGen processes cache steps (both automatic and manual), it:

1. Determines which backend to use based on configuration and runner type
2. Transforms the cache steps into appropriate commands for that backend
3. May replace them with custom commands or scripts

For example, a `restore_cache` step might become:

- CircleCI: Native `restore_cache` step (if using native backend)
- S3: Custom command using AWS CLI to download from S3
- Redis: Custom command to check Redis and download if found

## Best Practices

1. **Use built-in cache types** when possible - they handle most common scenarios
2. **Include both manifest and lock files** in checksum sources for safety
3. **Version detection files** should be checked into your repository
4. **Custom cache types** should follow the same patterns as built-in ones
5. **Cache names** should be descriptive (e.g., `gems`, `node_modules`, not `cache1`)

## Troubleshooting

### Cache Misses

If caches aren't being restored when expected:

1. Check that version files (`.ruby-version`, etc.) exist and are committed
2. Verify checksum source files haven't changed
3. Use `cigen inspect cache <job-name>` to see the generated cache keys

### Cache Conflicts

If different jobs need different cache strategies for the same dependency type:

1. Create custom cache types with different names
2. Or override cache definitions at the job level
