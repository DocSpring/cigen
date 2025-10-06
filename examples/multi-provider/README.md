# Multi-Provider Example

This demonstrates the core value proposition of CIGen: **write once, deploy everywhere**.

## The Problem

You want to use different CI providers for different purposes:

- **GitHub Actions**: PR checks, free for open source
- **CircleCI**: Production deployments, better resource classes
- **Buildkite**: Self-hosted runners for security-sensitive builds

Without CIGen, you maintain 3 separate configs in 3 different syntaxes. When you add a new test, you update all 3. When you change a cache key, you update all 3.

With CIGen: **one config, three outputs**.

## Single Source of Truth

```yaml
# cigen.yml
jobs:
  test:
    packages:
      - ruby
    steps:
      - run: bundle exec rspec
```

Run `cigen render` once → generates all three:

## Generated Outputs

### 1. GitHub Actions (`.github/workflows/ci.yml`)

```yaml
name: CI
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest

    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: postgres
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
        ports:
          - 5432:5432

      redis:
        image: redis:7
        ports:
          - 6379:6379

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Ruby
        uses: ruby/setup-ruby@v1
        with:
          ruby-version-file: .ruby-version
          bundler-cache: true

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version-file: .nvmrc
          cache: npm

      - name: Run tests
        run: bundle exec rspec
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost:5432/test
          REDIS_URL: redis://localhost:6379

      - name: Run npm tests
        run: npm test

  build:
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Build Docker image
        uses: docker/build-push-action@v5
        with:
          context: .
          push: false
          tags: myapp:latest
```

### 2. CircleCI (`.circleci/config.yml`)

```yaml
version: 2.1

orbs:
  slack: circleci/slack@4.12

executors:
  ruby-node:
    docker:
      - image: cimg/ruby:3.3-node
      - image: cimg/postgres:15
        environment:
          POSTGRES_USER: postgres
          POSTGRES_PASSWORD: postgres
      - image: cimg/redis:7

jobs:
  test:
    executor: ruby-node
    environment:
      DATABASE_URL: postgres://postgres:postgres@localhost:5432/test
      REDIS_URL: redis://localhost:6379
    steps:
      - checkout

      - restore_cache:
          keys:
            - gems-{{ checksum "Gemfile.lock" }}-v1
            - gems-

      - restore_cache:
          keys:
            - npm-{{ checksum "package-lock.json" }}-v1
            - npm-

      - run:
          name: Install dependencies
          command: |
            bundle install --deployment
            npm ci

      - save_cache:
          key: gems-{{ checksum "Gemfile.lock" }}-v1
          paths:
            - vendor/bundle

      - save_cache:
          key: npm-{{ checksum "package-lock.json" }}-v1
          paths:
            - node_modules

      - run:
          name: Run RSpec tests
          command: bundle exec rspec

      - run:
          name: Run npm tests
          command: npm test

  build:
    docker:
      - image: cimg/base:stable
    steps:
      - checkout
      - setup_remote_docker

      - run:
          name: Build Docker image
          command: |
            docker build -t myapp:latest .

workflows:
  version: 2
  ci:
    jobs:
      - test
      - build:
          requires:
            - test
```

### 3. Buildkite (`.buildkite/pipeline.yml`)

```yaml
agents:
  queue: default
  os: linux

steps:
  - label: ':rspec: Test'
    key: test
    env:
      DATABASE_URL: postgres://postgres@localhost:5432/test
      REDIS_URL: redis://localhost:6379
    plugins:
      - docker-compose#v4.16.0:
          run: app
          config:
            - docker-compose.test.yml
          env:
            - DATABASE_URL
            - REDIS_URL
      - cache#v1:
          paths:
            - vendor/bundle
          key: "gems-{{ checksum 'Gemfile.lock' }}"
      - cache#v1:
          paths:
            - node_modules
          key: "npm-{{ checksum 'package-lock.json' }}"
    command: |
      bundle install --deployment
      npm ci
      bundle exec rspec
      npm test

  - wait

  - label: ':docker: Build'
    depends_on: test
    plugins:
      - docker#v5.8.0:
          build: .
          image-name: myapp
          tag: latest
```

### 4. Auto-Generated Docker Compose (for Buildkite)

`.buildkite/docker-compose.test.yml`:

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_PASSWORD: postgres
    healthcheck:
      test: ['CMD-SHELL', 'pg_isready']
      interval: 10s

  redis:
    image: redis:7

  app:
    build: .
    depends_on:
      - postgres
      - redis
    environment:
      DATABASE_URL: postgres://postgres:postgres@postgres:5432/test
      REDIS_URL: redis://redis:6379
```

## Key Differences Handled Automatically

| Feature      | GitHub Actions     | CircleCI                 | Buildkite             |
| ------------ | ------------------ | ------------------------ | --------------------- |
| Services     | `services:` block  | Multi-image docker       | Docker Compose plugin |
| Caching      | actions/cache      | restore_cache/save_cache | cache plugin          |
| Ruby setup   | actions/setup-ruby | Built into image         | Docker image          |
| Dependencies | `needs:`           | `requires:`              | `depends_on:`         |
| Env vars     | `env:`             | `environment:`           | `env:`                |

CIGen abstracts all these differences into a single `cigen.yml`.

## Provider-Specific Features

### GitHub Actions Only

```yaml
provider_config:
  github:
    workflows:
      ci:
        pull_request_approval: true # Require approval before runs
        permissions:
          contents: read
          pull-requests: write
```

### CircleCI Only

```yaml
provider_config:
  circleci:
    orbs:
      - slack: circleci/slack@4.12
    resource_class: xlarge # Use bigger machines
```

### Buildkite Only

```yaml
provider_config:
  buildkite:
    agents:
      queue: priority # Route to specific agents
      os: linux
```

## Migration Path

### Scenario: Moving from CircleCI to GitHub Actions

**Before CIGen**:

1. Learn GitHub Actions syntax ⏱️ 1 week
2. Manually translate 20+ jobs ⏱️ 2 weeks
3. Debug subtle differences ⏱️ 1 week
4. **Total**: 4 weeks of work

**With CIGen**:

1. Already have `cigen.yml` ✅
2. Run `cigen render --provider github` ⏱️ 1 second
3. Commit `.github/workflows/` ⏱️ 5 minutes
4. **Total**: 5 minutes

## Testing Multiple Providers

CIGen itself uses this approach:

```bash
# Run the same test suite on all providers
cigen render --all

# Validate all generated configs
cigen validate --all

# Compare outputs
diff .github/workflows/ci.yml .circleci/config.yml  # (conceptually)
```

## Usage

```bash
# Generate for all configured providers
cigen render

# Generate specific provider only
cigen render --provider github
cigen render --provider circleci
cigen render --provider buildkite

# Generate for all providers (override config)
cigen render --all

# Preview what will be generated
cigen plan

# Validate all provider outputs
cigen validate --all
```

## Cost Savings Example

**Company with 100 developers**:

- **GitHub Actions**: $0.008/minute
- **CircleCI**: $0.015/minute
- **Buildkite**: Self-hosted, $15/agent/month

**Before**: Locked into one provider
**After**: Use cheapest provider for each workflow

- PR checks → GitHub (free for public repos)
- Production builds → Buildkite (self-hosted, secure)
- Savings: **~$2,000/month**

## The Dream: Provider Arbitrage

Future feature: Auto-route jobs to cheapest available provider:

```yaml
jobs:
  test:
    cost_optimization:
      prefer:
        - github # Free for open source
        - buildkite # Flat rate
      fallback: circleci # Pay per minute
```

CIGen generates configs for all three, CI runner picks cheapest available.
