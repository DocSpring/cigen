# Cigen Development Notes

## Design Decisions

### CircleCI Version Support

- **Decision**: Only support the latest CircleCI config version (2.1)
- **Rationale**: There's no benefit in supporting older config versions. This simplifies the codebase and encourages users to adopt current best practices.
- **Date**: December 2024

### Provider Architecture

- Providers are responsible for translating the internal object model to CI-specific configuration formats
- Each provider lives in its own module under `src/providers/`
- Providers implement a common trait to ensure consistency
