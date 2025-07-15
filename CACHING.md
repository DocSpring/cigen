# Cigen Caching System

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
- **checksum** - Combined hash of dependency files (e.g. package lock files)
  - If no exact match is found, the most recent version of the cache will be restored with the same name and versions.

## Built-in Cache Types

Cigen includes built-in definitions for common cache types:

### Ruby Gems

```yaml
caches:
  gems:
    version_sources:
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
    - command: "grep -A1 'RUBY VERSION' Gemfile.lock | tail -n1 | grep -o '[0-9]*\.[0-9]*\.[0-9]*' | head -1"
    - command: "ruby --version | grep -o '[0-9]*\.[0-9]*\.[0-9]*' | head -1"
  bundler:
    - command: "grep -A1 'BUNDLED WITH' Gemfile.lock | tail -n1 | tr -d ' '"
    - command: "bundler --version | grep -o '[0-9]*\.[0-9]*\.[0-9]*' | head -1"
```

Example cache key: `linux-ubuntu22.04-amd64-gems-ruby3.4.5-bundler2.6.3-abc123def456`

### Node Modules

```yaml
caches:
  node_modules:
    version_sources:
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
    - command: "node --version | grep -o '[0-9]*\.[0-9]*\.[0-9]*' | head -1"
  npm:
    - command: "npm --version | grep -o '[0-9]*\.[0-9]*\.[0-9]*' | head -1"
  yarn:
    - command: "yarn --version | grep -o '[0-9]*\.[0-9]*\.[0-9]*' | head -1"
  bun:
    - command: "bun --version | grep -o '[0-9]*\.[0-9]*\.[0-9]*' | head -1"
  pnpm:
    - command: "pnpm --version | grep -o '[0-9]*\.[0-9]*\.[0-9]*' | head -1"
```

Example cache key: `linux-ubuntu22.04-amd64-node_modules-node20.11.0-npm10.2.4-def456abc123`

### Python

```yaml
caches:
  pip:
    version_sources:
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
    - file: runtime.txt  # Heroku-style
    - command: "python --version 2>&1 | grep -o '[0-9]*\.[0-9]*\.[0-9]*' | head -1"
  pip:
    - command: "pip --version | grep -o '[0-9]*\.[0-9]*\.[0-9]*' | head -1"
```

Example cache key: `linux-ubuntu22.04-amd64-pip-python3.12.1-pip24.0-789abcdef012`

## Using Caches in Jobs

### Using Default Paths

In your job files, you can use the built-in cache types with their default paths:

```yaml
# workflows/test/jobs/install_gems.yml
cache:
  - gems # Uses default paths: vendor/bundle, .bundle
```

```yaml
# workflows/test/jobs/install_deps.yml
cache:
  - node_modules # Uses default path: node_modules
  - pip # Uses default paths: .venv, venv, ~/.cache/pip
```

### Overriding Default Paths

If you need different paths, you can override them:

```yaml
# workflows/test/jobs/install_gems.yml
cache:
  gems:
    - vendor/ruby # Custom path instead of vendor/bundle
    - .bundle
```

### Adding Additional Paths

You can also extend the defaults with additional paths:

```yaml
cache:
  node_modules:
    - node_modules # Default
    - .next/cache # Additional cache for Next.js
    - .turbo # Turborepo cache
```

This automatically generates a key like:

```
linux-ubuntu22.04-amd64-gems-ruby3.3.0-bundler2.5.6-abc123def456
```

## Customizing Cache Definitions

### Extending Built-in Caches

You can override or extend the built-in cache definitions in your `config.yml`:

```yaml
# .cigen/config.yml
caches:
  # Completely override the gems cache
  gems:
    version_sources:
      - file: .ruby-version
      - command: "ruby --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+'"
    checksum_sources:
      - Gemfile
      - Gemfile.lock
      - gems.locked  # Custom lockfile
    paths:
      - vendor/ruby   # Override default paths
      - .bundle
```

### Adding Custom Cache Types

Define new cache types for your specific needs:

```yaml
# .cigen/config.yml
caches:
  python_ml:
    version_sources:
      - file: .python-version
      - file: runtime.txt # Heroku-style
    checksum_sources:
      - requirements.txt
      - requirements-ml.txt
      - models/config.json
    paths:
      - .venv
      - models/cache
      - ~/.cache/torch

  go_modules:
    version_sources:
      - file: .go-version
      - file: go.mod
        pattern: '^go (.+)'
    checksum_sources:
      - go.sum
    paths:
      - ~/go/pkg/mod
      - .cache/go-build
```

### Using Custom Caches

```yaml
# In a job file
cache:
  # Use your custom cache type with default paths
  - python_ml

  # Or override the paths
  python_ml:
    - .venv
    - models/pretrained  # Different from default

  # Create an ad-hoc cache using an existing definition
  ml_data:
    - data/processed
    type: python_ml  # Reuse python_ml's version/checksum config
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
   - command: "ruby --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+'"
   ```

### Multiple Versions

If multiple languages are detected, they're concatenated:

```
linux-ubuntu22.04-amd64-myapp-ruby3.3.0-node20.1-abc123def
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

## Cache Backends

Different cache storage backends can be configured:

```yaml
# config.yml
caches:
  artifacts:
    backend: circleci # or s3, minio
    config:
      # backend-specific configuration
```

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
