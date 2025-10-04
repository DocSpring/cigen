# CIGen Plugin Architecture

## Vision

Transform CIGen into "the Terraform for CI config generation" - a unified, extensible platform with:

- **Tiny core** that orchestrates plugins via RPC
- **Plugin ecosystem** for providers (CircleCI, GitHub Actions, Buildkite, Jenkins) and modules (language support, caching, etc.)
- **Unified declarative schema** (`cig.toml`) that's provider-agnostic
- **Convention over configuration** with smart defaults
- **Work signature hashing** for intelligent job skipping across providers
- **No vendor lock-in** - write once, deploy to multiple CI platforms

## Core Architecture

### Three-Layer Model

```
┌─────────────────────────────────────────────────────────────┐
│                      CIGen Core (Rust)                      │
│  • Plugin discovery, spawn, handshake, versioning           │
│  • DAG construction from declarative schema                 │
│  • Capability registry & dependency resolution              │
│  • Work signature hashing & deterministic rendering         │
│  • File I/O, caching primitives, telemetry                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ gRPC over stdio
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
┌───────▼────────┐   ┌────────▼────────┐   ┌──────▼──────┐
│   Providers    │   │    Modules      │   │   Policy    │
├────────────────┤   ├─────────────────┤   ├─────────────┤
│ • github       │   │ • lang/ruby     │   │ • security  │
│ • circleci     │   │ • lang/node     │   │ • compliance│
│ • buildkite    │   │ • lang/python   │   │ • standards │
│ • jenkins      │   │ • db/postgres   │   └─────────────┘
│                │   │ • browser/chrome│
│ Each owns one  │   │ • cache/s3      │
│ target output  │   │ • runners/self  │
└────────────────┘   └─────────────────┘
```

### Core Responsibilities (Minimal)

The core is deliberately small and does ONLY:

1. **Plugin lifecycle**
   - Discovery (PATH, config, registry)
   - Spawn as separate processes
   - Handshake with version/capability negotiation
   - Crash isolation and error handling

2. **Schema parsing**
   - Load and validate `cig.toml`
   - Parse job definitions, matrices, dependencies
   - Resolve variable references and expressions

3. **DAG construction**
   - Build resource graph from plugin proposals
   - Dependency resolution (topological sort)
   - Detect cycles and conflicts
   - Matrix expansion and parameterization

4. **Work signature hashing**
   - Compute stable hashes from job inputs (files + env + versions)
   - Track changes for intelligent job skipping
   - Provider-agnostic caching of signatures

5. **Rendering & output**
   - Merge fragments from plugins by target namespace
   - Deterministic formatting (stable order, sorted keys)
   - Write to multiple output files
   - Generate diffs for review

6. **Telemetry & diagnostics**
   - Structured logging with tracing
   - Error aggregation from plugins
   - Performance metrics

### Everything Else is a Plugin

**Providers** (exclusive ownership of target configs):

- `provider/github-actions` → `.github/workflows/*.yml`
- `provider/circleci` → `.circleci/config.yml`
- `provider/buildkite` → `.buildkite/pipeline.yml`
- `provider/jenkins` → `Jenkinsfile`

**Modules** (composable features):

- `lang/ruby`, `lang/node`, `lang/python`, `lang/go`
- `db/postgres`, `db/mysql`, `db/redis`
- `browser/chromatic`, `browser/playwright`
- `cache/s3`, `cache/gcs`, `cache/minio`
- `runners/self-hosted`, `runners/aws-batch`

**Policy** (post-generation validation):

- `policy/security` - SOC 2 compliance checks
- `policy/cost-control` - Budget limits, resource quotas
- `policy/conventions` - Naming standards, required steps

## Plugin Protocol

### Transport: gRPC over stdio

- Core spawns plugin binaries as child processes
- Communication via gRPC messages on stdin/stdout
- stderr reserved for plugin logs/diagnostics
- Language-agnostic (plugins can be Rust, Go, TypeScript, Python, etc.)
- Crash isolation - plugin failures don't crash core

### Handshake & Capability Discovery

```protobuf
// Initial handshake
message Hello {
  uint32 core_protocol = 1;  // Protocol version (e.g., 3)
  string core_version = 2;    // Semantic version (e.g., "0.2.0")
}

message PluginInfo {
  string name = 1;                     // e.g., "provider/github"
  string version = 2;                  // e.g., "1.2.3"
  uint32 protocol = 3;                 // Must match core_protocol
  repeated string capabilities = 4;    // ["provider:github", "cache:native"]
  repeated string requires = 5;        // ["lang:*"] - dependencies on other plugins
  repeated string conflicts_with = 6;  // ["provider:*"] - mutual exclusions
  map<string, string> metadata = 7;    // Optional metadata
}
```

**Capabilities** are namespaced strings:

- `provider:github` - Can generate GitHub Actions config
- `lang:ruby` - Can detect and configure Ruby projects
- `cache:s3` - Can implement S3-based caching
- `step:docker` - Provides docker build steps
- `runner:self-hosted` - Manages self-hosted runners

**Requirements** enable plugin dependencies:

- `provider/circleci` might require `cache:*` (any cache backend)
- `lang/ruby` might require `step:install` (installation step support)

**Conflicts** prevent incompatible plugins:

- Multiple `provider:*` plugins conflict (only one can own an output target)
- `cache:s3` conflicts with `cache:gcs` for the same cache definition

### Plugin Hooks (Event-Driven Pipeline)

Plugins implement hooks that the core calls during the generation pipeline:

```protobuf
// 1. Detection: Scan repo and report signals
message DetectRequest {
  RepoSnapshot repo = 1;  // File tree, git info, etc.
}

message DetectResult {
  repeated string signals = 1;     // ["ruby_detected", "has_gemfile"]
  map<string, string> facts = 2;   // {"ruby_version": "3.3.0"}
  float confidence = 3;            // 0.0-1.0 score for auto-enable
}

// 2. Planning: Propose resources based on schema + signals
message PlanRequest {
  repeated string capabilities = 1;  // Available capabilities from all plugins
  map<string, string> facts = 2;     // Aggregated facts from detect phase
  CigSchema schema = 3;              // Parsed cig.toml
  map<string, string> flags = 4;     // CLI flags, env vars
}

message PlanResult {
  repeated Resource resources = 1;   // Jobs, steps, caches, secrets
  repeated Dependency deps = 2;      // "job:test" depends on "job:build"
}

// 3. Generation: Emit config fragments for target
message GenerateRequest {
  string target = 1;          // "github", "circleci", etc.
  repeated Resource graph = 2; // Final resolved DAG
  map<string, bytes> work_signatures = 3; // Job hash map for skip logic
}

message GenerateResult {
  repeated Fragment fragments = 1;
}

message Fragment {
  string path = 1;              // ".github/workflows/test.yml"
  string content = 2;           // YAML/JSON content
  MergeStrategy strategy = 3;   // REPLACE, MERGE, APPEND
  int32 order = 4;              // For APPEND strategy
}

// 4. Validation: Post-generation checks
message ValidateRequest {
  repeated Fragment rendered = 1;
}

message ValidateResult {
  repeated Diagnostic diagnostics = 1;
}

message Diagnostic {
  enum Level {
    ERROR = 0;
    WARNING = 1;
    INFO = 2;
  }
  Level level = 1;
  string code = 2;        // "SECURITY_001"
  string title = 3;
  string message = 4;
  string fix_hint = 5;    // Actionable suggestion
  SourceLocation loc = 6;
}

// 5. Preflight (optional): Decide if job should run
message PreflightRequest {
  string job_id = 1;
  RepoState repo_state = 2;
  bytes previous_signature = 3;
}

message PreflightResult {
  bool should_run = 1;
  string reason = 2;          // "files_changed", "forced", etc.
  bytes new_signature = 3;
}
```

### Hook Execution Flow

```
User runs: cig plan

1. DETECT
   Core → spawns all plugins → sends DetectRequest
   Plugins → scan repo → return DetectResult
   Core → aggregates signals & facts

2. PLAN
   Core → sends PlanRequest with schema + facts
   Plugins → propose resources (jobs, steps, caches)
   Core → builds DAG, resolves conflicts, sorts topologically

3. GENERATE
   Core → sends GenerateRequest with final graph
   Plugins → emit config fragments
   Core → merges fragments by namespace, sorts deterministically

4. VALIDATE
   Core → sends ValidateRequest with rendered output
   Plugins → check policy rules
   Core → aggregates diagnostics, fail on errors

5. OUTPUT
   Core → writes files, prints diff
```

## Unified Schema (cig.toml)

Provider-agnostic declarative config that replaces `.cigen/config.yml`:

```toml
version = "0.2"

[project]
name = "docspring"
default_runner = "linux-xlarge"

[variables]
RUBY_VERSION = "3.3"
NODE_VERSION = "20"

# Job definitions
[jobs.test]
needs = ["setup"]
matrix = { os = ["ubuntu-22.04"], ruby = ["3.2", "3.3"] }
packages = ["ruby", "node"]  # Auto-installs and caches
steps = [
  { run = "bundle exec rspec" }
]
skip_if = { paths_unmodified = ["app/**", "spec/**"], env = ["SKIP_TESTS"] }

[jobs.build]
needs = ["test"]
packages = ["docker"]
steps = [
  { uses = "docker/build@>=1.1", with = { target = "release", push = false } }
]

# Cache definitions (optional overrides)
[caches.bundler]
paths = ["vendor/bundle"]
key_parts = ["Gemfile.lock", "ruby:{{ ruby_version }}"]

[caches.node_modules]
paths = ["node_modules"]
key_parts = ["package-lock.json", "node:{{ node_version }}"]

# Runner definitions
[runners.linux-small]
github = { runs-on = "ubuntu-latest" }
circleci = { resource_class = "small" }

[runners.linux-xlarge]
github = { runs-on = "ubuntu-latest-8-cores" }
circleci = { resource_class = "xlarge" }

# Provider-specific outputs
[[outputs]]
provider = "github"
path = ".github/workflows/ci.yml"
description = "Main CI workflow"

[[outputs]]
provider = "circleci"
path = ".circleci/config.yml"
description = "CircleCI configuration"
```

### Key Features

**Packages** instead of manual cache config:

```toml
[jobs.test]
packages = ["ruby", "node"]  # Plugin detects versions, generates install + cache
```

**Matrix builds** with automatic expansion:

```toml
matrix = { ruby = ["3.2", "3.3"], arch = ["amd64", "arm64"] }
# Generates: test-ruby3.2-amd64, test-ruby3.2-arm64, test-ruby3.3-amd64, test-ruby3.3-arm64
```

**Conditional execution** (provider-agnostic):

```toml
skip_if = {
  paths_unmodified = ["app/**", "spec/**"],
  env = ["SKIP_TESTS"],
  branch = ["dependabot/*"]
}
```

**Uses directive** for modules:

```toml
steps = [
  { uses = "lang/ruby@~1.2", with = { version = "3.3" } },
  { uses = "db/postgres@^2.0", with = { version = "15" } },
  { run = "bundle exec rspec" }
]
```

## Work Signature Hashing (Job Skipping)

Core feature that makes CI fast by avoiding redundant work.

### Signature Components

For each job, compute a stable hash from:

1. **Source files** matching `skip_if.paths_unmodified` patterns
2. **Explicit inputs** declared in job definition
3. **Environment variables** listed in `skip_if.env`
4. **Dependency signatures** (hashes of upstream jobs)
5. **Package versions** (Ruby, Node, etc.)
6. **Job definition** itself (steps, commands, config)

### Provider-Specific Implementation

**CircleCI** (native support via setup workflows):

```yaml
# Generated .circleci/config.yml
version: 2.1
setup: true
workflows:
  setup:
    jobs:
      - compute-signatures:
          # Core generates this job
          # Computes hashes, compares to cache
          # Emits continuation config with only changed jobs
```

**GitHub Actions** (output-based conditional):

```yaml
# Generated .github/workflows/ci.yml
jobs:
  bootstrap:
    runs-on: ubuntu-latest
    outputs:
      test_changed: ${{ steps.sigs.outputs.test_changed }}
      build_changed: ${{ steps.sigs.outputs.build_changed }}
    steps:
      - run: cigen compute-signatures # Core provides this
      - id: sigs
        run: echo "test_changed=true" >> $GITHUB_OUTPUT

  test:
    needs: bootstrap
    if: needs.bootstrap.outputs.test_changed == 'true'
    # ... rest of job
```

**Buildkite** (dynamic pipeline):

```yaml
steps:
  - label: ':hash: Compute signatures'
    command: cigen compute-signatures | buildkite-agent pipeline upload
```

**Jenkins** (Job DSL):

```groovy
// seed job computes signatures
// generates downstream jobs conditionally
```

## Module System (Like Terraform Modules)

Modules are reusable, versioned plugin packages.

### Module Addressing

```
registry.cigen.dev/namespace/name@version
```

Examples:

- `registry.cigen.dev/lang/ruby@1.2.3` (official)
- `registry.cigen.dev/acme/custom-deploy@^2.0` (third-party)
- `github.com/user/repo//modules/my-module@v1.0.0` (git)

### Module Manifest

```toml
# lang/ruby/module.toml
[module]
name = "lang/ruby"
version = "1.3.0"
capabilities = ["step:setup", "cache:bundler", "detect:ruby"]

[inputs]
version = { type = "string", default = "3.3" }
bundler_version = { type = "string", optional = true }
install_path = { type = "string", default = "vendor/bundle" }

[provides]
caches = ["bundler"]
commands = ["bundle", "gem", "rake"]
```

### Using Modules

```toml
# In cig.toml
[jobs.test]
steps = [
  { uses = "lang/ruby@~1.2", with = { version = "3.3" } },
  { run = "bundle exec rspec" }
]
```

Core resolves version constraints, downloads module, and invokes its plugin during the plan phase.

### Module Composition

Modules can depend on other modules:

```toml
# lang/rails/module.toml
[module]
name = "lang/rails"
version = "2.0.0"
requires = ["lang/ruby@~1.0", "db/postgres@^2.0"]
```

## Lockfile & Reproducibility

### cig.lock Format

```toml
# Auto-generated, do not edit
schema_version = "1"

[core]
version = "0.2.0"
protocol = 3

[plugins."provider/github"]
version = "1.2.3"
sha256 = "abc123..."
source = "registry.cigen.dev/provider/github"

[plugins."lang/ruby"]
version = "1.3.0"
sha256 = "def456..."
source = "registry.cigen.dev/lang/ruby"

[resolved_versions]
ruby = "3.3.0"
bundler = "2.6.3"
node = "20.11.0"
```

### Workflow

1. `cig init` - Creates `cig.toml`
2. `cig sync` - Resolves versions, downloads plugins, generates `cig.lock`
3. `cig plan` - Uses locked versions for deterministic builds
4. `cig render` - Generates final CI configs

## Migration Path from Current CIGen

### Phase 1: Protocol & Core (Week 1-2)

1. Define protobuf schemas for plugin protocol
2. Implement plugin manager in core:
   - Discovery (PATH, config)
   - Spawn + handshake
   - Hook invocation (detect, plan, generate, validate)
3. Build minimal "passthrough" plugin for GitHub Actions
   - Takes current internal generator
   - Wraps in plugin protocol
   - Validates end-to-end flow

### Phase 2: Schema Migration (Week 2-3)

1. Design `cig.toml` schema
2. Implement parser and validator
3. Create migration tool: `.cigen/config.yml` → `cig.toml`
4. Update core to use new schema format
5. Maintain backward compatibility during transition

### Phase 3: Extract Providers (Week 3-4)

1. Extract CircleCI generator → `provider/circleci` plugin
2. Extract GitHub Actions → `provider/github` plugin
3. Implement Buildkite provider as greenfield
4. Each provider is a separate binary on PATH
5. Core discovers and orchestrates via protocol

### Phase 4: Module System (Week 4-5)

1. Extract language support → `lang/*` modules
   - `lang/ruby` (Gemfile, .ruby-version detection)
   - `lang/node` (package.json, .nvmrc detection)
   - `lang/python` (requirements.txt, .python-version)
2. Extract cache backends → `cache/*` modules
   - `cache/s3`, `cache/gcs`, `cache/minio`
3. Implement module resolution and versioning
4. Create local plugin directory structure

### Phase 5: Work Signature & Skipping (Week 5-6)

1. Implement signature computation in core
2. Add signature storage (local cache, remote cache)
3. Generate skip logic for each provider:
   - CircleCI: setup workflow + continuation
   - GitHub: bootstrap job + conditionals
   - Buildkite: dynamic pipeline
4. Test and validate skip behavior

### Phase 6: Registry & Ecosystem (Week 6+)

1. Design plugin registry API
2. Implement registry server (simple HTTP + storage)
3. Add `cig publish` command for plugin authors
4. Create plugin SDK and templates
5. Document plugin development process
6. Build example third-party plugins

## Standard Library Plugins

Ship with core, version-locked, but still separate binaries:

**Providers** (std/providers/):

- `provider/github`
- `provider/circleci`
- `provider/buildkite`
- `provider/jenkins`

**Languages** (std/lang/):

- `lang/ruby`
- `lang/node`
- `lang/python`
- `lang/go`
- `lang/rust`

**Services** (std/services/):

- `db/postgres`
- `db/mysql`
- `db/redis`
- `browser/chromatic`

**Infrastructure** (std/infra/):

- `cache/s3`
- `cache/gcs`
- `runners/self-hosted`

These are installed with core but can be overridden by specifying explicit versions in `cig.toml`.

## Testing Strategy

### Golden Tests

```
tests/golden/
  ruby_app/
    input/
      cig.toml
      app/
      spec/
    expected/
      github/workflows/ci.yml
      circleci/config.yml
      buildkite/pipeline.yml
```

Run: `cig render --all` and compare output to expected.

### Multi-Provider E2E

CIGen's own CI runs the SAME `cig.toml` across all supported providers:

- GitHub Actions (primary)
- CircleCI (test)
- Buildkite (test)
- Jenkins (test)

This validates:

1. Schema is truly provider-agnostic
2. Skip logic works on all platforms
3. Providers maintain feature parity

### Plugin Conformance Suite

Define required hooks and behaviors:

- Provider must implement: detect, plan, generate
- Provider must handle skip_if correctly
- Provider must support matrix builds

Gate provider releases on passing conformance tests.

## Security Model

### Process Isolation

Plugins run as separate processes with:

- Read-only repo snapshot (copy or bind-mount)
- Write access only to designated temp dir
- No network access by default (opt-in via capability)

### Capability System

Plugins declare required capabilities in manifest:

```toml
[module]
capabilities_required = ["filesystem:read", "network:https"]
```

Core enforces at runtime. User must approve high-risk capabilities.

### WASM Path (Future)

For untrusted community plugins:

- Compile to WASM component (WIT interface)
- Core embeds Wasmtime runtime
- Same hook API, but sandboxed execution
- No filesystem or network access unless explicitly granted

## CLI User Experience

### Commands

```bash
# Initialize new project
cig init

# Sync dependencies (like terraform init)
cig sync

# Plan changes (show DAG, diffs)
cig plan

# Generate configs (like terraform apply)
cig render

# Generate for specific provider
cig render --provider github

# Explain why a job will/won't run
cig explain job test

# Validate without generating
cig validate

# Compute work signatures (used internally by CI)
cig compute-signatures

# Publish a plugin
cig publish --plugin ./my-plugin

# Add a plugin dependency
cig plugin add lang/ruby@~1.2
```

### Output Format

```
$ cig plan

Detecting project...
  ✓ Ruby 3.3.0 detected (lang/ruby)
  ✓ Node 20.11.0 detected (lang/node)
  ✓ PostgreSQL 15 declared (db/postgres)

Resolving dependencies...
  ✓ provider/github@1.2.3
  ✓ lang/ruby@1.3.0
  ✓ lang/node@2.0.1
  ✓ db/postgres@2.1.0

Planning resources...
  + job.setup (runs on linux-small)
  + job.test (matrix: 4 variants)
  + job.build (runs on linux-xlarge)

Generating...
  ✓ .github/workflows/ci.yml (432 lines)

Changes:
  + 1 workflow
  + 6 jobs
  ~ 2 caches updated

Run `cig render` to apply these changes.
```

## Benefits of This Architecture

1. **Extensibility** - Anyone can write plugins in any language
2. **Modularity** - Core stays small, features are composable
3. **Testability** - Each plugin tested independently
4. **Stability** - Plugin crashes don't crash core
5. **Portability** - Same config works across all CI providers
6. **Performance** - Skip logic drastically reduces CI time
7. **Ecosystem** - Registry enables community contributions
8. **Vendor Freedom** - Switch CI providers without rewriting configs
9. **Dogfooding** - CIGen uses itself for its own CI

## Risks & Mitigations

**Risk**: gRPC overhead for many small plugins
**Mitigation**: Batch operations, keep hot plugins loaded, profile

**Risk**: Protocol versioning hell
**Mitigation**: Strict semver, clear deprecation policy, lockfiles

**Risk**: Plugin discovery complexity
**Mitigation**: Simple PATH-based discovery, explicit config, registry

**Risk**: Community plugins with bugs/exploits
**Mitigation**: WASM sandbox, capability system, review process

**Risk**: Breaking changes during migration
**Mitigation**: Parallel systems during transition, feature flags, migration tool

## Next Steps

1. Review and approve this architecture document
2. Create detailed protobuf schema definitions
3. Build minimal plugin manager prototype
4. Extract one provider (GitHub) as proof of concept
5. Validate end-to-end flow with real project
6. Iterate based on learnings

This transformation will take ~6-8 weeks but will result in a fundamentally more powerful and flexible system.
