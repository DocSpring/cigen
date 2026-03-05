# Woodpecker CI Example

This example demonstrates using cigen to generate Woodpecker CI configurations for use with self-hosted Gitea + Woodpecker CI.

## Use Case

- **GitHub**: Uses GitHub Actions for public CI/CD
- **Gitea (self-hosted)**: Uses Woodpecker CI for private builds on your home server

Same configuration, two different CI providers!

## Configuration

```yaml
providers:
  - woodpecker # or add github for multi-provider setup

project:
  name: woodpecker-demo
```

Jobs are defined in `.cigen/workflows/ci/jobs/`:

- `test.yml` - Run tests
- `lint.yml` - Run linting
- `build.yml` - Build release (depends on test + lint)

## Generate Woodpecker Config

```bash
cigen generate --config .cigen
```

This generates `.woodpecker/ci.yaml` with all your workflow steps.

## Multi-Provider Setup

To generate for both GitHub Actions and Woodpecker CI:

```yaml
providers:
  - github
  - woodpecker
```

Then run `cigen generate` to create both:

- `.github/workflows/ci.yml` (for GitHub)
- `.woodpecker/ci.yaml` (for Gitea)

Push to GitHub → GitHub Actions runs
Push to Gitea → Woodpecker CI runs

**Same config, different providers!**

## Woodpecker CI Features

The generated configuration supports:

- Sequential and parallel steps
- Step dependencies (`needs`)
- Multiple workflows
- Container images
- Environment variables
- Conditional execution

## Next Steps

- Add services (PostgreSQL, Redis, etc.)
- Configure matrix builds
- Add labels for agent selection
- Set up deployment workflows
