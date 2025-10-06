# CIGen Examples

This directory contains example `cigen.yml` configurations demonstrating different use cases and complexity levels.

## Quick Start

Pick the example that matches your needs:

### 1. **Minimal** - First-time users

Perfect if you want to:

- Get started with zero boilerplate
- See convention-over-configuration in action
- Generate a simple test workflow

**Complexity**: ⭐☆☆☆☆
**Time to implement**: 2 minutes
**Lines of config**: 5

[View minimal example →](./minimal/)

### 2. **Rails App** - Production applications

Perfect if you have:

- Multiple languages (Ruby + Node.js)
- Database and cache services
- Matrix builds across versions
- Docker builds and deployments
- Complex skip logic

**Complexity**: ⭐⭐⭐⭐☆
**Time to implement**: 15 minutes
**Lines of config**: ~100 (vs. ~300 with manual YAML)

[View Rails app example →](./rails-app/)

### 3. **Monorepo** - Large codebases

Perfect if you have:

- Nx or Turborepo setup
- 10+ apps/libraries
- Need selective job execution
- Want 10x faster CI

**Complexity**: ⭐⭐⭐☆☆
**Time to implement**: 20 minutes
**Lines of config**: ~50

[View monorepo example →](./monorepo/)

### 4. **Multi-Provider** - Platform agnostic

Perfect if you want to:

- Generate configs for multiple CI platforms
- Migrate between providers easily
- Use different providers for different workflows
- Avoid vendor lock-in

**Complexity**: ⭐⭐⭐⭐☆
**Time to implement**: 10 minutes
**Lines of config**: ~60

[View multi-provider example →](./multi-provider/)

## Comparison Matrix

| Feature              | Minimal   | Rails App | Monorepo      | Multi-Provider |
| -------------------- | --------- | --------- | ------------- | -------------- |
| Single language      | ✅        | ❌        | ❌            | ✅             |
| Multiple languages   | ❌        | ✅        | ✅            | ✅             |
| Services (DB, Redis) | ❌        | ✅        | ✅            | ✅             |
| Matrix builds        | ❌        | ✅        | ✅            | ❌             |
| Skip logic           | ✅ (auto) | ✅        | ✅ (advanced) | ✅             |
| Docker builds        | ❌        | ✅        | ❌            | ✅             |
| Deployments          | ❌        | ✅        | ✅            | ❌             |
| Nx integration       | ❌        | ❌        | ✅            | ❌             |
| Multiple providers   | ❌        | ❌        | ❌            | ✅             |

## Feature Showcase

### Convention Over Configuration

```yaml
# This minimal config:
jobs:
  test:
    packages:
      - ruby
# Auto-generates:
# ✅ Checkout step
# ✅ Ruby setup with version detection
# ✅ Bundle install
# ✅ Gem caching with Gemfile.lock key
# ✅ Skip logic for unchanged files
```

### Matrix Builds

```yaml
matrix:
  ruby:
    - '3.2'
    - '3.3'
  arch:
    - amd64
    - arm64
# Generates 4 jobs automatically
```

### Smart Skip Logic

```yaml
skip_if:
  paths_unmodified:
    - app/**
    - spec/**
# If no files in these paths changed:
# ✅ Skip job entirely (not just cache hit)
# ✅ Save CI time and cost
# ✅ Works across all providers
```

### Module System

```yaml
steps:
  - uses: docker/build@>=1.1
    with:
      push: false
      tags: myapp:latest
# Reusable, versioned modules
# Like GitHub Actions, but provider-agnostic
```

### Multi-Provider Output

```yaml
providers:
  - github
  - circleci
  - buildkite
# One config → three outputs
# ✅ No vendor lock-in
# ✅ Easy migration
# ✅ Use cheapest provider per workflow
```

## Common Patterns

### Basic Test Job

```yaml
jobs:
  test:
    packages:
      - ruby
    steps:
      - run: bundle exec rspec
```

### With Database

```yaml
jobs:
  test:
    packages:
      - ruby
    services:
      - postgres:15
    env:
      DATABASE_URL: postgres://postgres@localhost/test
    steps:
      - run: bundle exec rake db:schema:load
      - run: bundle exec rspec
```

### With Caching

```yaml
jobs:
  test:
    packages:
      - ruby
    steps:
      - run: bundle exec rspec

# Caching is automatic!
# But you can customize:
caches:
  bundler:
    paths:
      - vendor/bundle
    key_parts:
      - Gemfile.lock
      - ruby:{{ ruby_version }}
```

### With Docker

```yaml
jobs:
  build:
    packages:
      - docker
    steps:
      - uses: docker/build@1.0
        with:
          context: .
          push: false
          tags: myapp:latest
```

### Deployment

```yaml
jobs:
  deploy:
    trigger: manual  # Workflow dispatch
    # or
    trigger:
      tags: v*  # On git tags only
    steps:
      - run: ./deploy.sh production
```

## Learning Path

1. **Start with Minimal** - Understand the basics
2. **Add complexity** - Services, matrix, skip logic
3. **Try Monorepo** - If you have many projects
4. **Go Multi-Provider** - When you need flexibility

## Migration Guides

### From GitHub Actions

```yaml
# Before (GitHub Actions)
name: CI
on: [push]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: ruby/setup-ruby@v1
        with:
          ruby-version: 3.3
          bundler-cache: true
      - run: bundle exec rspec

# After (CIGen)
jobs:
  test:
    packages:
      - ruby
    steps:
      - run: bundle exec rspec
```

**Savings**: 12 lines → 5 lines (58% reduction)

### From CircleCI

```yaml
# Before (CircleCI)
version: 2.1
jobs:
  test:
    docker:
      - image: cimg/ruby:3.3
    steps:
      - checkout
      - restore_cache:
          keys:
            - gems-{{ checksum "Gemfile.lock" }}
      - run: bundle install --deployment
      - save_cache:
          key: gems-{{ checksum "Gemfile.lock" }}
          paths:
            - vendor/bundle
      - run: bundle exec rspec
workflows:
  ci:
    jobs: [test]

# After (CIGen) - same as above
jobs:
  test:
    packages:
      - ruby
    steps:
      - run: bundle exec rspec
```

**Savings**: 18 lines → 5 lines (72% reduction)

## Next Steps

1. **Pick an example** that matches your use case
2. **Copy the `cigen.yml`** to your repo
3. **Customize** for your needs
4. **Run `cigen plan`** to preview
5. **Run `cigen render`** to generate

## Getting Help

- 📖 [Full documentation](https://cigen.dev/docs)
- 💬 [GitHub Discussions](https://github.com/docspring/cigen/discussions)
- 🐛 [Report issues](https://github.com/docspring/cigen/issues)
- 🌐 [cigen.dev](https://cigen.dev)

## Contributing Examples

Have a great example? Submit a PR!

Requirements:

- Include `cigen.yml`
- Include `README.md` explaining use case
- Keep it focused on one pattern/feature
- Show what gets auto-generated
