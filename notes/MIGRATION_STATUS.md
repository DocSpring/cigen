# Plugin Architecture Migration Status

## Overview

CIGen is being transformed from a monolithic Rust binary into a plugin-based architecture inspired by Terraform. This will enable true multi-provider support, extensibility, and intelligent CI optimization.

**Target Architecture**: "The Terraform for CI Config Generation"

## Completed Work (Phase 1 - In Progress)

### ✅ Documentation & Planning

**PLUGIN_ARCHITECTURE.md** - Complete architectural vision (400+ lines)

- Three-layer model (core + plugins + policy)
- gRPC-over-stdio communication protocol
- Plugin lifecycle: detect → plan → generate → validate → preflight
- Unified `cigen.yml` schema (provider-agnostic)
- Work signature hashing for job skipping
- Module system with versioning and lockfiles
- Security model with process isolation
- Migration strategy and risk mitigation

**PROJECT_PLAN.md** - 6-phase migration roadmap

- Phase 1: Protocol & Plugin Manager (current, ~2 weeks)
- Phase 2: Schema Migration to cigen.yml (~1 week)
- Phase 3: Extract Providers as Plugins (~1 week)
- Phase 4: Module System (~1 week)
- Phase 5: Work Signature & Job Skipping (~1 week)
- Phase 6: Registry & Ecosystem (~1+ week)
- Success metrics and testing strategy defined

### ✅ Protocol Definition

**proto/plugin.proto** - Complete gRPC service definition

- Handshake messages (Hello, PluginInfo)
- Capability negotiation system
- Hook definitions:
  - Detect: Repository scanning and signal detection
  - Plan: Resource proposal and dependency resolution
  - Generate: Config fragment emission
  - Validate: Post-generation policy checks
  - Preflight: Job skip decision (signature-based)
- Rich diagnostic and error reporting
- Version negotiation protocol

### ✅ Build Infrastructure

**Cargo Workspace** - Structured for core + plugins

```
cigen/
  Cargo.toml (workspace root)
  src/ (core library + CLI binary)
  plugins/
    provider-github/
      Cargo.toml
      src/main.rs
```

**Dependencies Added**:

- `tonic` - gRPC runtime (0.14.2)
- `prost` - Protobuf encoding (0.14.1)
- `tonic-prost` - Integration layer (0.14.2)
- `tonic-prost-build` - Build-time code generation (0.14.2)

**build.rs** - Automatic protobuf compilation

- Generates Rust types from proto/plugin.proto
- Output: `cigen.plugin.v1` module with all message types

### ✅ Core Plugin Infrastructure

**src/plugin/** - Plugin system modules

- `protocol.rs` - Auto-generated protobuf types
- `manager.rs` - Plugin lifecycle orchestration (stub)
- `discovery.rs` - Plugin discovery mechanisms (stub)
- `stdio_transport.rs` - Custom gRPC-over-stdio transport (stub)

**Status**: All compiles successfully, tests pass

### ✅ First Plugin: GitHub Actions Provider

**plugins/provider-github** - Standalone plugin binary

- Implements full gRPC Plugin service trait
- Handshake with capability declaration
- Stubs for all hooks (detect, plan, generate, validate, preflight)
- Version: 0.1.0, Protocol: 1
- Capabilities: `provider:github`, `cache:native`, `matrix:build`
- Conflicts: `provider:*` (exclusive provider ownership)

**Status**: Compiles successfully as separate binary

## Current State

### What Works

- ✅ Protobuf schemas compile
- ✅ Workspace builds (core + GitHub provider plugin)
- ✅ All existing tests still pass
- ✅ Plugin declares capabilities and metadata
- ✅ Foundation for stdio communication in place

### What's Missing (Phase 1 Remaining)

1. **stdio Transport Implementation**
   - Need to wire up ChildChannel to tonic's transport layer
   - Implement message framing (length-prefixed messages)
   - Handle bidirectional streaming over pipes

2. **Plugin Manager Implementation**
   - Process spawning with stdio pipes
   - Handshake invocation and verification
   - Hook dispatching to active plugins
   - Error handling and crash recovery
   - Graceful shutdown

3. **GitHub Provider Integration**
   - Wire existing `providers/github_actions` generator
   - Map internal models to protobuf messages
   - Generate actual YAML fragments
   - Return proper diagnostics

4. **Core Orchestration**
   - Integrate PluginManager into generate command
   - Route generation requests to plugins
   - Merge fragments from multiple plugins
   - Write final output files

5. **End-to-End Testing**
   - Test: core spawns plugin
   - Test: handshake succeeds
   - Test: generate hook produces valid YAML
   - Test: output matches golden files

## Architecture Highlights

### Plugin Communication Flow

```
┌──────────────┐                    ┌───────────────────┐
│              │   1. spawn process │                   │
│              │   with stdio pipes │                   │
│  CIGen Core  ├───────────────────►│  Plugin Process   │
│              │                    │                   │
│              │   2. Hello         │                   │
│              ├───────────────────►│                   │
│              │                    │                   │
│              │   3. PluginInfo    │                   │
│              │◄───────────────────┤                   │
│              │                    │                   │
│              │   4. GenerateReq   │                   │
│              ├───────────────────►│                   │
│              │                    │ • calls existing  │
│              │                    │   generator code  │
│              │   5. GenerateRes   │ • emits YAML      │
│              │◄───────────────────┤                   │
│              │                    │                   │
│              │   6. shutdown      │                   │
│              ├───────────────────►│                   │
└──────────────┘                    └───────────────────┘
      │
      │ writes merged output
      ▼
.github/workflows/ci.yml
```

### Provider Plugin Responsibilities

Each provider plugin:

- **Detects** repository signals (e.g., .github/ directory exists)
- **Plans** resources (jobs, steps, caches) from schema
- **Generates** provider-specific YAML/JSON config
- **Validates** output against provider schemas
- **Computes** job signatures for skip logic

**Key Insight**: Providers are _isolated processes_ - crashes don't take down core, versioning is independent, and anyone can write plugins in any language.

### Unified Schema Vision

Current (`.cigen/config.yml`):

```yaml
# Provider-specific, lots of boilerplate
workflows:
  test:
    jobs:
      - name: test
        # Manual cache configuration
        # Provider-specific steps
```

Future (`cigen.yml`):

```toml
[jobs.test]
needs = ["setup"]
packages = ["ruby", "node"]  # Auto-installs, auto-caches
steps = [
  { run = "bundle exec rspec" }
]
skip_if = { paths_unmodified = ["app/**", "spec/**"] }
```

**Same config generates** `.github/workflows/ci.yml` **AND** `.circleci/config.yml` **AND** `.buildkite/pipeline.yml`

## Next Steps (Immediate)

### Week 1-2: Complete Phase 1

**Priority 1: stdio Transport**

- Implement message framing (4-byte length prefix + protobuf)
- Create `StdioClientChannel` for core → plugin communication
- Create `StdioServerChannel` for plugin to accept connections
- Test bidirectional message passing

**Priority 2: Plugin Manager**

- Implement `spawn()` - fork plugin process, capture stdio
- Implement `handshake()` - send Hello, verify PluginInfo
- Implement `invoke_generate()` - call generate hook
- Add error handling and timeouts

**Priority 3: GitHub Provider**

- Move `src/providers/github_actions/generator.rs` logic into plugin
- Convert `WorkflowConfig` to protobuf `GenerateRequest`
- Convert output to `Fragment` messages
- Return proper YAML content

**Priority 4: Integration**

- Update `generate_command()` to use PluginManager
- Discover GitHub provider plugin on PATH
- Spawn, handshake, and invoke for generation
- Write fragments to files

**Priority 5: Testing**

- Integration test: spawn plugin, handshake, shutdown
- Integration test: generate workflow, compare to golden file
- Update existing snapshot tests to work with plugin system

### Success Criteria for Phase 1

- [ ] `cigen generate` spawns `cigen-provider-github` plugin
- [ ] Handshake succeeds with version/capability exchange
- [ ] Plugin generates `.github/workflows/ci.yml`
- [ ] Output matches current generator (golden test passes)
- [ ] All existing tests continue to pass

## Future Phases (Preview)

**Phase 2**: Migrate to `cigen.yml` schema
**Phase 3**: Extract CircleCI as second provider, validate multi-provider
**Phase 4**: Language modules (`lang/ruby`, `lang/node`)
**Phase 5**: Job skipping via work signatures (the killer feature!)
**Phase 6**: Public registry, community plugins, ecosystem growth

## Benefits of This Architecture

1. **Multi-Provider**: Write once, deploy to GitHub, CircleCI, Buildkite, Jenkins
2. **Extensibility**: Anyone can write plugins (Rust, Go, TypeScript, Python)
3. **Performance**: Job skipping reduces CI time by 50%+ (DocSpring goal)
4. **Vendor Freedom**: Switch CI platforms without rewriting configs
5. **Modularity**: Core stays tiny (~5K lines), plugins are independently versioned
6. **Security**: Process isolation, capability system, optional WASM sandbox
7. **Ecosystem**: Registry enables community contributions (like Terraform)
8. **Dogfooding**: CIGen will use itself for its own multi-provider CI

## Risks & Mitigations

**Risk**: stdio transport complexity slows progress
**Mitigation**: Use simple length-prefixed framing, refer to existing implementations (e.g., go-plugin)

**Risk**: Migration breaks existing DocSpring config
**Mitigation**: Maintain backward compatibility, run both systems in parallel during transition

**Risk**: Plugin overhead too high
**Mitigation**: Keep hot plugins loaded, batch operations, profile early

**Risk**: Community doesn't adopt
**Mitigation**: Focus on excellent stdlib plugins, showcase DocSpring as proof of value

## Timeline

- **Week 1-2**: Complete Phase 1 (plugin protocol working end-to-end)
- **Week 3**: Phase 2 (cigen.yml schema)
- **Week 4**: Phase 3 (multi-provider validated)
- **Week 5**: Phase 4 (module system)
- **Week 6**: Phase 5 (job skipping - the prize!)
- **Week 7+**: Phase 6 (registry, polish, docs)

**Goal**: Production-ready plugin architecture in 6-8 weeks

## Questions & Decisions Needed

1. **Message Framing**: Use simple 4-byte length prefix? Or more sophisticated (e.g., gRPC wire format)?
2. **Plugin Discovery**: PATH-only for now, or also check `.cigen/plugins/`?
3. **Backward Compat**: Keep old generator as fallback, or clean break in v0.2.0?
4. **Testing Strategy**: Golden tests vs runtime comparison vs both?

## Resources

- **Architecture Doc**: `notes/PLUGIN_ARCHITECTURE.md`
- **Project Plan**: `notes/PROJECT_PLAN.md`
- **Protocol**: `proto/plugin.proto`
- **Example Plugin**: `plugins/provider-github/`

---

**Last Updated**: October 5, 2025
**Status**: Phase 1 in progress (40% complete)
**Next Milestone**: stdio transport implementation
