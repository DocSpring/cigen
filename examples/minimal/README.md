# Minimal Example

This demonstrates the absolute minimum CIGen configuration - convention over configuration.

## Config

```yaml
jobs:
  test:
    packages:
      - ruby
    steps:
      - run: bundle exec rspec
```

## What Happens Automatically

With just 5 lines of config, CIGen automatically:

### Detection

- ✅ Detects Ruby version from `.ruby-version` or `Gemfile`
- ✅ Detects that this is a test job (by name convention)

### Installation & Caching

- ✅ Generates `bundle install --deployment` step
- ✅ Creates cache for `vendor/bundle`
- ✅ Keys cache on `Gemfile.lock` checksum

### Job Steps

- ✅ Adds checkout step (first)
- ✅ Adds restore cache step (after checkout)
- ✅ Adds install step (bundle install)
- ✅ Adds your test command (bundle exec rspec)
- ✅ Adds save cache step (after tests)

### Skip Logic

- ✅ Skips job if source files unchanged
- ✅ Default patterns: `app/**`, `lib/**`, `spec/**`, `Gemfile*`

### Multi-Provider Output

From this single config, CIGen generates:

**GitHub Actions** (`.github/workflows/ci.yml`):

```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/cache@v3
        with:
          path: vendor/bundle
          key: ${{ runner.os }}-gems-${{ hashFiles('Gemfile.lock') }}
      - run: bundle install --deployment
      - run: bundle exec rspec
```

**CircleCI** (`.circleci/config.yml`):

```yaml
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
          paths: [vendor/bundle]
      - run: bundle exec rspec
workflows:
  version: 2
  ci:
    jobs: [test]
```

**Buildkite** (`.buildkite/pipeline.yml`):

```yaml
steps:
  - label: 'Test'
    command: bundle exec rspec
    plugins:
      - cache#v1:
          paths: ['vendor/bundle']
          key: "gems-{{ checksum 'Gemfile.lock' }}"
```

## Usage

```bash
# Generate configs for all providers
cigen render

# Generate for specific provider only
cigen render --provider github
cigen render --provider circleci
cigen render --provider buildkite
```

## Key Principles

1. **Zero boilerplate** - No workflow definitions, no manual cache config
2. **Conventions** - Smart defaults based on job names and language detection
3. **Multi-provider** - Same config works for GitHub, CircleCI, Buildkite, etc.
4. **Auto-optimization** - Skip logic and caching configured automatically
