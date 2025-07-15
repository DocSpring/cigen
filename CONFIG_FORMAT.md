# CIGen Configuration Format

CIGen uses its own configuration format that compiles to CircleCI, GitHub Actions, and other CI providers. This document explains the key differences and design decisions.

## Philosophy

Our format follows these principles:

- **Intuitive over provider-specific**: Use concepts that make sense to developers, not CI providers
- **DRY (Don't Repeat Yourself)**: Reduce duplication through better abstractions
- **Least surprise**: Support multiple syntaxes where it makes sense (inspired by Ruby/Rails)
- **Provider-agnostic**: Avoid leaking provider-specific concepts into the configuration

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

**CIGen (multiple supported formats):**

```yaml
cache:
  # Simple string
  vendor: vendor/bundle

  # Array shorthand
  gems:
    - vendor/bundle
    - .bundle

  # Full control
  assets:
    restore: false
    paths: public/assets
```

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

**Why:** CircleCI's cache steps are verbose and repetitive. Our config format treats caching as a first-class feature and supports multiple intuitive syntaxes.

### 5. Workflow Discovery

**CIGen:**

```
workflows/
├── test/
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

### 7. Template Support

**CIGen:**

```yaml
image: cimg/postgres:{{ postgres_version }}
steps:
  - name: Set hosts
    run: |
      echo "{{ read('etc-hosts.txt') | trim }}" >> /etc/hosts
```

**Why:** First-class template support using Jinja2-style syntax reduces duplication and enables dynamic configuration.

### 8. Schema Validation

**CIGen:**

```yaml
$schema: ../../schemas/v1/config-schema.json
provider: circleci
```

**Why:** Built-in schema validation ensures configurations are correct before generation, providing immediate feedback in editors like VS Code and Cursor.

## Migration

When migrating from CircleCI or GitHub Actions:

1. Service containers become first-class definitions
2. Complex cache logic becomes simple cache declarations
3. Workflow definitions become directory structures
4. Repetitive auth configurations become centralized
5. Verbose step definitions become concise name/run pairs
