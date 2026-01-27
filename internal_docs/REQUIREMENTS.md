# DocSpring CircleCI Migration Requirements

## CRITICAL: Architectural Philosophy

**This is NOT a 1:1 port of the Ruby system!**

cigen represents a complete architectural redesign based on lessons learned from the Ruby implementation. Key principles:

1. **Convention over Configuration**: Common patterns (cache keys, job naming, dependencies) are handled by the cigen engine, not manual template code
2. **Declarative over Imperative**: Jobs declare what they need (caches, services, dependencies), not HOW to implement them
3. **DRY Templates**: Templates contain ONLY what makes each workflow/job unique - all boilerplate is eliminated
4. **Smart Defaults**: The system infers sensible defaults from context rather than requiring explicit configuration
5. **Separation of Concerns**: Business logic lives in the cigen engine, not in templates

### What This Means for Migration

The Ruby system's complex ERB helpers like `sha256_cache_exists()`, `update_cache_exists()`, etc. are NOT ported over. Instead:

- **Cache management**: Declared in job definitions, handled automatically by cigen
- **Architecture matrix**: Declared once, applied systematically
- **Job dependencies**: Inferred from file patterns and explicit declarations
- **Resource allocation**: Based on job type and architecture conventions

Templates become clean, readable YAML with minimal logic - just variable substitution and simple conditionals where truly needed.

## Overview

DocSpring currently uses a Ruby-based ERB templating system to generate multiple CircleCI configuration files. The migration to cigen will transform this into a cleaner, convention-based system.

## Current DocSpring System

### File Generation

The Ruby script (`lib/tools/generate_circle_ci_config.rb`) generates multiple output files from ERB templates:

**Active/Used Configs:**

1. **config.yml** (from `setup_config.yml.erb`) - Main entry point with dynamic configuration (setup workflow)
2. **test_and_deploy_config.yml** (from `test_and_deploy_config.yml.erb`) - Main workflow with all test and deployment jobs
3. **package_updates_config.yml** (from `package_updates_config.yml.erb`) - Scheduled/triggered package update checks
4. **staging_postman_tests_config.yml** (from `staging_postman_tests_config.yml.erb`) - Scheduled/triggered staging API tests

**Unused/Legacy (DO NOT MIGRATE):**

- **test_config.yml** (from `test_config.yml.erb`) - UNUSED
- **build_config.yml** (from `build_config.yml.erb`) - UNUSED

### Key Features Used

#### 1. ERB Templating

- Variable interpolation: `<%= @variable %>`
- Conditional blocks: `<% if condition %>`
- Loops and iteration: `<% array.each do |item| %>`
- Method calls: `<%= render('template_name') %>`
- Custom helper methods for cache management, job definitions, etc.

#### 2. Dynamic Configuration

- Uses CircleCI's `setup: true` for dynamic workflow generation
- The setup job determines which config file to use based on pipeline parameters
- Supports conditional workflow execution

#### 3. Multi-Architecture Support

- Jobs can be generated for both `amd64` and `arm64`
- Architecture-specific resource classes and Docker images
- Job names get architecture suffixes (e.g., `install_gems_amd64`, `install_gems_arm64`)

#### 4. Cache Management

- SHA256-based cache keys for dependencies
- Cache existence tracking across jobs
- Automatic cache key generation from file patterns
- Version-based cache invalidation

#### 5. Job Dependencies and Requirements

- Jobs declare requirements (other jobs that must complete first)
- Automatic dependency graph construction
- Support for ignored dependencies and contexts

#### 6. Custom Helper Methods

The Ruby script provides many helper methods:

- `job()` - Declare job metadata (requirements, context, build status)
- `sha256_cache_exists()` - Check for cache based on file checksums
- `update_cache_exists()` - Update cache existence tracking
- `render()` - Render nested templates
- `render_all()` - Render all templates in a directory
- `indent_lines()` - Maintain proper YAML indentation

## Required cigen Features

### 1. Template Engine Support

**Priority: Critical**
**Status: ✅ ALREADY IMPLEMENTED**

cigen already has minijinja integrated as the templating engine, which provides:

- Support for conditionals (`if`/`else`) ✅
- Support for loops (`for`/`each`) ✅
- Support for method/function calls ✅
- Support for including/rendering other templates ✅
- Variable scoping and context passing ✅

**What's needed:**

- Add more helper functions to match DocSpring's ERB helpers
- Implement template file loading from `.cigen/templates/` directory
- Add support for partial templates and includes

### 2. Multi-Output Generation

**Priority: Critical**

cigen must support generating multiple output files from a single command:

```yaml
# .cigen/config.yml
outputs:
  - template: setup_config.yml.j2
    output: .circleci/config.yml
    description: 'Setup workflow with dynamic configuration'
  - template: test_and_deploy_config.yml.j2
    output: .circleci/test_and_deploy_config.yml
    description: 'Main workflow with test and deployment jobs'
  - template: package_updates_config.yml.j2
    output: .circleci/package_updates_config.yml
    description: 'Scheduled workflow for package updates'
  - template: staging_postman_tests_config.yml.j2
    output: .circleci/staging_postman_tests_config.yml
    description: 'Triggered workflow for staging API tests'
```

### 3. Dynamic Configuration Support

**Priority: Critical**

Support CircleCI's dynamic configuration pattern:

- Generate setup workflows with `setup: true`
- Support continuation orb usage
- Conditional config file selection based on parameters

### 4. Architecture Matrix Support

**Priority: High**

Support generating jobs for multiple architectures:

```yaml
architectures:
  - amd64
  - arm64

jobs:
  install_gems:
    matrix:
      arch: ${architectures}
    # Generates: install_gems_amd64, install_gems_arm64
```

### 5. Advanced Cache Management

**Priority: High**

Implement cache key generation and tracking:

- SHA256 checksum generation from file patterns
- Cache existence tracking across jobs
- Version-based cache invalidation
- Cache backend abstraction (as originally planned)

### 6. Job Dependency Resolution

**Priority: High**

Enhanced job dependency management:

- Support for job requirements declaration
- Automatic dependency graph construction
- Topological sorting for workflow generation
- Support for conditional dependencies

### 7. Template Helper Functions

**Priority: Medium**

**IMPORTANT: This is NOT a 1:1 port of the Ruby helpers!**

The cigen architecture removes boilerplate through convention over configuration. Cache keys, job names, and other patterns are handled by the cigen engine based on declarations in the job definitions, NOT through manual template functions.

Minimal helper functions needed:

- `include(template_name)` - Include another template
- Basic filters for string manipulation (already in minijinja)

### 8. Environment Variable Integration

**Priority: Medium**

Support environment variables in templates:

- Read from shell environment
- Support `.env` files
- Override via CLI flags
- Default values in config

### 9. Conditional Workflow Generation

**Priority: Medium**

Support conditional workflow generation based on:

- Git branch patterns
- Environment variables
- Pipeline parameters
- File changes (via git diff)

### 10. Package Version Management

**Priority: Low**

Support for package version detection:

- Run version detection scripts
- Parse version strings
- Use versions in cache keys
- Support multiple version sources

## Implementation Plan

### Phase 1: Multi-Output Support (Immediate)

1. ✅ Template engine already integrated (minijinja)
2. Extend config format for multiple outputs
3. Modify generator to handle multiple files
4. Add CLI support for selecting specific outputs

### Phase 2: Core Conventions (Week 1)

1. Implement architecture matrix support (convention-based)
2. Add automatic cache key generation from job definitions
3. Implement smart job dependency resolution
4. Add service container conventions

### Phase 3: Dynamic Configuration (Week 1-2)

1. Add support for CircleCI's `setup: true` pattern
2. Implement conditional config file selection
3. Add pipeline parameter handling
4. Support continuation orb pattern

### Phase 4: Template Conversion (Week 2)

1. Convert DocSpring job definitions to cigen format
2. Create clean Jinja2 templates (minimal logic)
3. Define conventions for DocSpring patterns
4. Validate against original outputs

### Phase 5: Testing & Polish (Week 2-3)

1. Ensure generated configs match originals functionally
2. Optimize for readability (remove unnecessary generated cruft)
3. Document the new conventions
4. Create migration guide

## Success Criteria

1. cigen can generate the 4 active CircleCI config files that match the current Ruby-generated output:
   - config.yml (setup workflow)
   - test_and_deploy_config.yml (main workflow)
   - package_updates_config.yml (scheduled)
   - staging_postman_tests_config.yml (triggered)
2. All DocSpring CI features are supported (caching, multi-arch, dynamic config)
3. Generated configs pass CircleCI validation
4. Migration path is documented
5. Performance is equal or better than Ruby script

## Notes

- The current Ruby script is ~900 lines and uses heavy ERB templating
- DocSpring has ~50 job templates in `.circleci/src/ci_jobs/`
- The system uses complex cache key generation with SHA256 checksums
- Job dependencies are automatically tracked and validated
- The setup config conditionally loads different configs based on pipeline parameters
