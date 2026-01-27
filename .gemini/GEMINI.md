# Gemini Context & Memories

## Project: cigen / docspring migration

### Core Concepts & "The New Way"

- **cigen vs. Old Config:** We are migrating DocSpring's CircleCI configuration from a Ruby-based generator (`generate_circle_ci_config.rb`) to `cigen`. The old config logic is the "source of truth" for intended behavior (job logic), but `cigen` manages the structure, caching, and hashing.
- **Auto-Injection & Caching:** `cigen` automatically wraps jobs with logic to compute a `JOB_HASH` based on source files and configuration. It handles caching (`save_cache`, `restore_cache` for job status) and skipping jobs that have already passed for a given hash.
- **No Manual Hashing:** We should **not** see manual `calculate_sha256_hash` or `update_cache_exists` steps in the new `cigen` YAML files for standard jobs. This logic is now handled by `cigen`'s auto-injection.
- **Source File Groups:** `source_files` keys in job definitions refer to groups defined in `docspring/.cigen/config/source_file_groups.yml`. This is how `cigen` knows which files to hash for a job.
- **Configuration Layering:** `cigen` config is merged from `docspring/.cigen/config.yml` and `docspring/.cigen/config/*`.

### Key Corrections & Rules

- **CRITICAL RULE: No Inter-Stage Job Dependencies:** Jobs within different stages **cannot** directly depend on each other. Dependencies are managed at the **stage level** (stages depend on other stages). If Job A requires Job B, they **must** be in the same stage.
- **Job Naming Convention & Stages:**
  - **Correct Stages:**
    - `ci` (Linting, Testing, Docs Install/Lint/Compile)
    - `build` (App & Enterprise Docker Images, Manifests)
    - `deploy_staging` (Staging Deploy & Postman)
    - `deploy_docs` (Docs Deploy)
    - `deploy_us`, `deploy_eu`, `deploy_all` (Production Deploys)
    - `post_deploy` (Merge Branch)
  - **Noun Redundancy Convention:** If a stage name contains a noun (e.g., `deploy_docs`), the job file should describe the verb/action without repeating the noun (e.g., `deploy.yml` -> `deploy_docs_deploy`).
- **`docspring/.cigen/workflows/main/jobs/deploy/`:** This folder is specifically for **multi-stage Rails app deployment jobs (staging, US, EU)** using a matrix. Docs-related deployment jobs (`deploy_docs`) do _not_ belong here.
- **`cigen_shallow_checkout`**: This step is **automatically injected** by `cigen`. It should **not** be explicitly listed as a step in job definitions.
- **Docs Isolation & Monorepo Optimization:**
  - Docs are isolated. `docs_lint` and `compile_docs` (renamed `compile`) are in the `ci` stage (for fast feedback/blocking bad merges), but `build_app_image` does **not** depend on them.
  - `deploy_docs` is in its own stage.

### Enterprise Image Pipeline

- **Multi-Arch:** We build `amd64` and `arm64` images for both App and Enterprise.
- **Jobs:**
  - `build_app_image` (Multi-arch, in `build`)
  - `create_app_docker_manifest` (Requires `build_app_image`, pushes manifest)
  - `build_enterprise_image` (Multi-arch, in `build`, requires `build_app_image`)
  - `create_enterprise_docker_manifest` (Requires `build_enterprise_image`, pushes manifest)
- **Ordering:** `deploy_staging` depends on `build` stage, ensuring all images and manifests are ready before deployment/merge.

### Key Commands

- **Build cigen:** `cargo build --release` (run in root)
- **Generate config:** `cd docspring && ../target/release/cigen`
- **Compare configs:** `source .venv/bin/activate && ./scripts/compare_ci_configs.py` (requires `pyyaml`)
