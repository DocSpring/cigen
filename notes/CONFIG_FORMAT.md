# CIGen Configuration Format

CIGen uses its own configuration format that compiles to CircleCI, GitHub Actions, and other CI providers. This document explains the key differences and design decisions.

## Philosophy

Our format follows these principles:

- **Intuitive over provider-specific**: Use concepts that make sense to developers
- **DRY (Don't Repeat Yourself)**: Reduce duplication through better abstractions
- **Least surprise**: Support multiple syntaxes where it makes sense (inspired by Ruby/Rails)
- **Provider-agnostic**: Avoid leaking provider-specific concepts into the configuration
- **Convention over Configuration**: Smart defaults based on common patterns

## Key Differences

### 1. Steps Format

**CIGen:**

```yaml
steps:
  - name: Run tests
    run: |
      bundle exec rspec
  - name: Upload coverage
    run: coverage-reporter upload
```

**CircleCI:**

```yaml
steps:
  - run:
      name: Run tests
      command: |
        bundle exec rspec
  - run:
      name: Upload coverage
      command: coverage-reporter upload
```

**Why:** Our format is cleaner and more readable. The `name` and `run` keys are at the same level, reducing nesting and making the intent clearer.

### 2. Service Containers

**CIGen:**

```yaml
# In config.yml
services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_PASSWORD: test

# In job file
services:
  - postgres
  - redis
```

**CircleCI:**

```yaml
docker:
  - image: cimg/ruby:3.2 # Primary container mixed with services
  - image: postgres:15
    environment:
      POSTGRES_PASSWORD: test
  - image: redis:7
```

**GitHub Actions:**

```yaml
services:
  postgres:
    image: postgres:15
    env:
      POSTGRES_PASSWORD: test
```

**Why:** CircleCI's design mixes the primary container with service containers in a single array, where order matters. This is confusing and error-prone. We adopted GitHub Actions' cleaner `services` concept that clearly separates concerns.

### 3. Docker Authentication

**CIGen:**

```yaml
docker:
  default_auth: docker_hub
  auth:
    docker_hub:
      username: $DOCKERHUB_USERNAME
      password: $DOCKERHUB_TOKEN
    ghcr:
      username: $GITHUB_ACTOR
      password: $GITHUB_TOKEN
```

**CircleCI:**

```yaml
# Repeated for every image
docker:
  - image: myimage
    auth:
      username: $DOCKERHUB_USERNAME
      password: $DOCKERHUB_TOKEN
```

**Why:** Centralizing auth configuration with a default reduces repetition and makes it easier to manage credentials.

### 4. Cache Definitions

**CIGen:**

```yaml
# Using built-in cache definitions
cache: gems  # Automatically injects save/restore cache steps

# Or multiple caches
cache:
  - node_modules
  - pip

# Or with path overrides
cache:
  gems:
    paths:
      - vendor/ruby  # Override default paths
      - .bundle
```

**Built-in cache definitions include:**

- Intelligent version detection (Ruby, Node.js, Python, etc.)
- Automatic cache key generation based on lock files
- Platform-specific paths with fallback detection
- Automatic injection of cache restore/save steps in the job

**CircleCI:**

```yaml
- restore_cache:
    keys:
      - v1-gems-{{ checksum "Gemfile.lock" }}
- save_cache:
    key: v1-gems-{{ checksum "Gemfile.lock" }}
    paths:
      - vendor/bundle
```

**Why:** CircleCI's cache steps are verbose and repetitive. CIGen provides intelligent defaults while supporting customization. Cache keys automatically include OS, architecture, runtime versions, and file checksums.

### 4a. Manual Cache Steps

**CIGen also supports manual cache steps with cross-platform syntax:**

```yaml
steps:
  - restore_cache:
      name: Restore custom cache
      key: my-custom-{{ checksum "config.json" }}

  - name: Build something
    run: npm run build

  - save_cache:
      name: Save custom cache
      key: my-custom-{{ checksum "config.json" }}
      paths:
        - dist/
        - .cache/
```

**Key differences from native providers:**

1. **Same syntax everywhere** - Works identically for CircleCI, GitHub Actions, etc.
2. **Swappable backends** - Based on your cache configuration, these steps might use:
   - Native CI provider cache (default)
   - S3/MinIO buckets
   - Redis cache
   - Custom cache plugins
3. **Automatic backend selection** - CIGen chooses the appropriate backend based on:
   - Runner type (cloud vs self-hosted)
   - Cache backend configuration
   - Environment variables

**Example with backend configuration:**

```yaml
# In config.yml
cache_backends:
  default: native
  self_hosted: s3

  s3:
    bucket: my-cache-bucket
    region: us-east-1

# Manual cache steps automatically use the configured backend
steps:
  - restore_cache:
      key: my-build-{{ checksum "package-lock.json" }}
```

When compiled for CircleCI with self-hosted runners, this might generate custom commands that use S3 instead of CircleCI's native cache.

### 5. Workflow Discovery and Configuration

**CIGen:**

```
workflows/
├── test/
│   ├── config.yml      # Optional: workflow-specific config
│   └── jobs/
│       ├── rspec.yml
│       └── lint.yml
└── deploy/
    └── jobs/
        └── production.yml
```

**CircleCI:**

A single config.yml file:

```yaml
workflows:
  test:
    jobs:
      - rspec
      - lint
  deploy:
    jobs:
      - production
```

**Why:** Directory structure provides natural organization and discovery. No need to maintain a separate workflow definition - if the directory exists, the workflow exists.

#### Workflow Output Configuration

CIGen supports flexible output file generation:

**Single file output (default):**

```yaml
# In root config.yml
output_path: .circleci # Optional, defaults to .circleci for the CircleCI provider
output_filename: config.yml # Optional, defaults to config.yml
```

This generates all workflows in a single `.circleci/config.yml` file.

**Split file output:**

```yaml
# In workflows/setup/config.yml
output_filename: config.yml  # CircleCI requires an initial .circleci/config.yml file
dynamic: false  # Static workflow

# In workflows/test/config.yml
output_filename: dynamic_config.yml
dynamic: true   # Dynamic workflow with job skipping
```

This generates separate files:

- `.circleci/config.yml` - Static setup workflow that CircleCI runs first
- `.circleci/dynamic_config.yml` - Dynamic workflow with job skipping

**Dynamic workflows:** Setting `dynamic: true` enables intelligent job skipping based on file changes:

- Jobs are marked as successful for specific file checksums
- Subsequent runs skip unchanged jobs and reuse cached artifacts
- Configurable cache backend configuration (native CI cache, Redis, S3, etc.)

The default is `dynamic: false`.

### 6. File Organization

**CIGen:**

```
.cigen/
├── config.yml          # Main configuration
├── config/            # Optional: split configuration
│   ├── services.yml
│   └── caches.yml
├── commands/          # Reusable command templates
└── workflows/         # Workflow definitions
```

**Why:** Clear separation of concerns with optional splitting for complex configurations (similar to Terraform).

### 7. Reusable Commands

**CIGen:**

```yaml
# In commands/install_ruby.yml
steps:
  - name: Install Ruby dependencies
    run: |
      bundle config set frozen 'true'
      bundle install --jobs 4 --retry 3

# In job file
steps:
  - install_ruby
  - name: Run tests
    run: bundle exec rspec
```

**CircleCI:**

```yaml
commands:
  install_ruby:
    steps:
      - run:
          name: Install Ruby dependencies
          command: |
            bundle config set frozen 'true'
            bundle install --jobs 4 --retry 3

jobs:
  test:
    steps:
      - install_ruby
      - run:
          name: Run tests
          command: bundle exec rspec
```

**Why:** Commands are defined as separate files, making them easier to find, share, and maintain. The simpler syntax reduces boilerplate.

### 8. Template Support

**CIGen:**

```yaml
image: cimg/postgres:{{ postgres_version }}
steps:
  - name: Set hosts
    run: |
      echo "{{ read('etc-hosts.txt') | trim }}" >> /etc/hosts
```

**Why:** First-class template support using Jinja2-style syntax (MiniJinja template engine) reduces duplication and enables dynamic configuration.

### 9. Architecture Support

**CIGen:**

```yaml
# In config.yml
architectures: ["amd64", "arm64"]

resource_classes:
  amd64:
    small: small
    medium: medium
    large: large
  arm64:
    small: arm.small
    medium: arm.medium
    large: arm.large

# In job file
architectures: ["amd64", "arm64"]  # Run on both
resource_class: medium  # Automatically maps to correct provider class
```

**CircleCI:**

```yaml
# Must manually specify for each job
jobs:
  test-amd64:
    resource_class: medium
  test-arm64:
    resource_class: arm.medium
```

**Why:** CIGen abstracts away provider-specific resource class naming and makes multi-architecture builds simple.

### 10. Schema Validation

**CIGen:**

```yaml
$schema: ../../schemas/v1/config-schema.json
provider: circleci
```

**Why:** Built-in schema validation ensures configurations are correct before generation, providing immediate feedback in editors like VS Code and Cursor.

## Multi-Output Support

CIGen can generate multiple output files from a single configuration, useful for CircleCI's dynamic configuration pattern:

### Configuration

```yaml
# .cigen/cigen.yml
provider: circleci
outputs:
  - template: setup_workflow.yml.j2
    output: .circleci/config.yml
    description: 'Setup workflow with dynamic configuration'
  - template: main_workflow.yml.j2
    output: .circleci/test_and_deploy_config.yml
    description: 'Main test and deployment workflow'
  - template: scheduled_updates.yml.j2
    output: .circleci/package_updates_config.yml
    description: 'Scheduled package update checks'
```

### CLI Usage

```bash
# Generate all outputs
cigen generate

# Generate specific output
cigen generate --output .circleci/config.yml

# List available outputs
cigen list-outputs
```

### Template Organization

Templates are stored in `.cigen/templates/` and can include partials:

```
.cigen/
├── cigen.yml
├── templates/
│   ├── setup_workflow.yml.j2
│   ├── main_workflow.yml.j2
│   ├── scheduled_updates.yml.j2
│   └── partials/
│       ├── _commands.yml.j2
│       └── _docker_images.yml.j2
```

## Migration

When migrating from CircleCI or GitHub Actions:

1. **Service containers** become first-class definitions
2. **Cache steps** are replaced with built-in cache definitions:
   - `restore_cache`/`save_cache` → `cache: gems`
   - Manual cache key construction → Automatic version detection
3. **Workflow definitions** become directory structures
4. **Docker auth** configurations become centralized
5. **Step definitions** use cleaner name/run syntax
6. **Commands** move to separate files for better organization
7. **Multi-architecture** support through simple configuration
8. **Multiple config files** are generated from templates with shared partials
