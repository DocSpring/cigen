# Woodpecker CI Provider Plugin

Provider plugin for generating Woodpecker CI workflow configurations.

## Status

✅ **Working**: Basic workflow generation with DocSpring production workload (43 jobs, 3 workflows, 723-line config)

## Supported Features

### Core Features (Implemented)

- ✅ Basic step conversion (run commands)
- ✅ Multiple workflows (generates separate `.woodpecker/*.yaml` files)
- ✅ Job dependencies (`needs`)
- ✅ Container images
- ✅ Environment variables
- ✅ Commands execution
- ✅ Conditional execution (basic `when` conversion)

### Tested With

- ✅ Simple example (3 jobs: test, lint, build)
- ✅ DocSpring production config (43 jobs across 3 workflows)

## Not Yet Implemented

### High Priority

- ⏳ Services (PostgreSQL, Redis, etc.)
- ⏳ Native cache steps (currently only in-script caching)
- ⏳ Matrix builds
- ⏳ Labels for agent selection
- ⏳ Woodpecker plugins (`uses` step conversion)

### Medium Priority

- ⏳ Step names (currently all steps are unnamed)
- ⏳ Workspace configuration
- ⏳ Secrets management
- ⏳ Clone configuration
- ⏳ Detached steps (background processes)
- ⏳ `skip_clone` support

### Low Priority

- ⏳ Privileged mode
- ⏳ DNS configuration
- ⏳ Advanced when conditions (path filters, cron, etc.)
- ⏳ Step failure modes

## Architecture

The plugin converts cigen's generic schema to Woodpecker CI YAML:

```
CigenSchema → build_workflow_fragments() → Fragment[]
    ↓
JobDefinition[] → build_steps_sequence() → Step YAML
    ↓
RunStep → convert_run_step() → Woodpecker step mapping
```

## Output Structure

Generated files follow Woodpecker CI conventions:

```
.woodpecker/
  ├── ci.yaml          # Main CI workflow
  ├── deploy.yaml      # Deployment workflow
  └── package_updates.yaml  # Package update checks
```

## Example Conversion

### Input (cigen)

```yaml
jobs:
  test:
    image: rust:latest
    steps:
      - run: cargo test
```

### Output (Woodpecker CI)

```yaml
steps:
  - image: rust:latest
    commands:
      - cargo test
```

## Testing

Run tests:

```bash
cargo test -p cigen-provider-woodpecker
```

Current coverage: 6 unit tests covering:

- Basic step conversion
- Environment variables
- Multiple jobs
- Multiple workflows
- Workflow rendering

## Known Limitations

1. **Services**: Not implemented - service containers are ignored
2. **Cache**: Native Woodpecker cache actions not generated
3. **Step Names**: All steps are currently unnamed (Woodpecker will auto-number them)
4. **Uses Steps**: GitHub Actions-style `uses` steps are skipped
5. **Matrix**: Matrix builds not yet supported
6. **When Conditions**: Only basic `if` → `evaluate` conversion

## Future Enhancements

### Phase 1: Essential Features

- Add services support (databases, caches)
- Implement step names
- Add Woodpecker cache plugin steps

### Phase 2: Advanced Features

- Matrix builds
- Labels for agent targeting
- Clone configuration
- Workspace settings

### Phase 3: Polish

- Better `when` condition conversion
- Secrets management
- Privileged mode support
- Detached steps

## Integration with DocSpring

Successfully tested with DocSpring's production configuration:

- **43 jobs** across 3 workflows
- **723 lines** of generated Woodpecker YAML
- All jobs converted without errors

Example usage for multi-provider setup (GitHub + Woodpecker):

```yaml
providers:
  - github # GitHub Actions for public CI
  - woodpecker # Woodpecker CI for self-hosted Gitea
```

## Development

To add new features:

1. Add conversion logic in `build_workflow_fragments()` or `convert_run_step()`
2. Add tests in the `#[cfg(test)]` module
3. Test with both simple examples and DocSpring config
4. Update this README

## Resources

- [Woodpecker CI Docs](https://woodpecker-ci.org/docs/)
- [Workflow Syntax](https://woodpecker-ci.org/docs/usage/workflow-syntax)
- [Plugin Protocol](../../proto/plugin.proto)
