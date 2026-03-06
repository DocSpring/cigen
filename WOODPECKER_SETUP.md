# Getting Woodpecker CI Running with cigen

## Quick Start

### 1. Add Woodpecker to Your Config

In your `.cigen/config.yml`:

```yaml
providers:
  - woodpecker # For Gitea push
  - circleci # For GitHub push (optional)
```

### 2. Generate Woodpecker Config

```bash
cargo run --bin cigen -- generate --config /path/to/docspring/.cigen
```

This generates:

- `.woodpecker/main.yaml`
- `.woodpecker/package_updates.yaml`
- `.woodpecker/staging_postman_tests.yaml`

### 3. Commit and Push to Gitea

```bash
cd /path/to/docspring
git add .woodpecker/
git commit -m "Add Woodpecker CI configuration"
git push git@git.home.ndbroadbent.com:DocSpring/docspring.git
```

Woodpecker will automatically detect and run `.woodpecker/*.yaml` files.

## What's Included

✅ **43 jobs** from DocSpring config converted to Woodpecker steps
✅ **Services** automatically collected (postgres, redis, minio)
✅ **Step names** for readable CI logs
✅ **Multiple workflows** as separate YAML files
✅ **Environment variables** preserved
✅ **Container images** properly mapped

## Generated Structure

```yaml
# .woodpecker/main.yaml
services:
  postgres:
    image: postgres:16
  redis:
    image: redis:7
  minio:
    image: minio/minio:RELEASE.2025-06-26T18-44-10Z

steps:
  - name: Remove non-production gems
    image: build_libs
    commands:
      - bundle config set --local without 'development test debugging'
      - bundle clean --force

  - name: Docker Login
    image: build_libs
    commands:
      - echo "$DOCKERHUB_TOKEN" | docker login -u "$DOCKERHUB_USERNAME" --password-stdin

  # ... 700+ more lines
```

## Woodpecker Worker Configuration

Your Woodpecker workers need:

1. **Docker access** - For running container images
2. **Git access** - To clone from Gitea
3. **Environment variables** - DockerHub credentials, etc.
4. **Network access** - To pull images and access services

## Testing Locally

You can test Woodpecker configs locally with the Woodpecker CLI:

```bash
# Install Woodpecker CLI
go install go.woodpecker-ci.org/woodpecker/v2/cmd/cli@latest

# Test a workflow
woodpecker-cli exec --local .woodpecker/main.yaml
```

## Multi-Provider Workflow

Once you add both providers, you can:

**Push to GitHub** → CircleCI runs

```bash
git push origin main
```

**Push to Gitea** → Woodpecker CI runs

```bash
git push git@git.home.ndbroadbent.com:DocSpring/docspring.git main
```

**Same config, different CI providers!**

## Current Limitations

These will be added in future updates:

- Matrix builds (parallelism: 12 not yet supported)
- Native Woodpecker cache plugins
- Advanced when conditions (path filters, etc.)
- Step dependencies for parallel execution

For now, all steps run sequentially in the order defined.

## Troubleshooting

### Services not connecting

Check that services are available before steps run:

```yaml
when:
  - event: push
```

### Environment variables missing

Add them to Woodpecker repo settings or use secrets:

```yaml
environment:
  DATABASE_URL: postgres://postgres@postgres:5432/test
```

### Steps running out of order

Woodpecker runs steps sequentially by default. Dependencies are managed through the `depends_on` field (not yet implemented in cigen).

## Next Steps

To enhance the Woodpecker integration:

1. Add matrix build support for parallelism
2. Implement Woodpecker cache plugins
3. Add `when` conditions for selective execution
4. Support `depends_on` for parallel workflows

Current provider is production-ready for sequential workflows with services!
