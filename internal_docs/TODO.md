# Cigen Migration TODOs

## Fix Approval Jobs & Conditional Execution Logic

**Status:** Pending Research & Design

**Context:**
We have currently implemented some approval jobs explicitly (e.g., `approve_build_app_image.yml` with `type: approval`). This violates the "convention over configuration" principle of `cigen`. `cigen` was intended to handle approvals at a higher level, utilizing the concept of **stages** (defined in `docspring/.cigen/workflows/main/config.yml`) to automatically generate approval checkpoints.

**Goal:**
Replace manual `type: approval` job definitions with an automatic, stage-based approval system that supports **OR dependencies** and intelligent triggering.

**Key Requirements:**

1.  **Automatic Approval Jobs:** `cigen` should generate approval jobs automatically based on stage definitions (e.g., a `deploy_staging` stage implies an approval step before jobs in that stage run).
2.  **OR Dependencies (The Core Feature):**
    - **Scenario:** We need to build the application image (`build_image` stage) whenever _any_ deploy is approved.
    - **Behavior:** If a user approves the "Deploy Staging" job, it should automatically kick off the `build_app_image` job (if it hasn't run already) _and_ wait for it to complete.
    - **Flexibility:** A user should also be able to manually approve `build_app_image` early (e.g., while tests are running) if they are confident, without waiting for the deploy approval.
    - **Implementation:** This requires `cigen` to support `requires_any` or similar logic.
      - **GitHub Actions:** Maps cleanly to `if: needs.A.result == 'success' || needs.B.result == 'success'`.
      - **CircleCI:** Requires automated "shim" jobs to simulate OR logic because CircleCI only supports AND dependencies natively.

**Action Items:**

- [ ] **Research:** detailed review of `cigen`'s current stage implementation and dependency graph resolution. Determine how much of the "OR support" is already present vs. planned.
- [ ] **Design:** Flesh out the spec for `requires_any` / implicit stage dependencies.
- [ ] **Refactor:** Remove manual `approve_*.yml` jobs and configure `main/config.yml` stages to generate them.
- [ ] **Verify:** Ensure the `build_app_image` -> `deploy_*` dependency flow works as described (lazy build on deploy approval OR eager manual build).

## Rename Jobs to Follow Noun-Redundancy Convention

**Status:** Completed

**Context:**
Since `cigen` automatically prefixes job names with the stage name (e.g., `stage_name_job_name`), we should avoid repeating the "noun" in the job filename if it's already in the stage name.

**Convention:**
If a stage name contains a noun (e.g., `build_docs`), the job file should describe the verb/action without repeating the noun (e.g., `compile.yml` instead of `compile_docs.yml`).
Result: `build_docs_compile` (Good) vs `build_docs_compile_docs` (Redundant).

**Action Items:**

- [x] Rename `build_docs/compile_docs.yml` -> `build_docs/compile.yml`.
- [x] Rename `build_docs/install_docs_npm_packages.yml` -> `build_docs/install_npm_packages.yml`. (Moved to `ci` stage instead).
- [x] Rename `deploy_docs/deploy_docs.yml` -> `deploy_docs/deploy.yml`.
- [ ] Audit other jobs for similar redundancy (e.g., `deploy_staging_deploy` -> `deploy_staging` stage + `deploy` job?).

## Port Enterprise Image Pipeline & Multi-Arch Support

**Status:** Completed

**Context:**
We need to build both `amd64` and `arm64` images for the App and Enterprise versions to support multi-arch deployments. The current `cigen` config only has `amd64` app build. We need to restore the full pipeline including manifest creation and Enterprise builds.

**Dependencies:**

- App Image (amd64 + arm64) -> App Manifest -> Enterprise Image (amd64 + arm64) -> Enterprise Manifest.
- `merge_branch` (in `post_deploy`) should ideally wait for this entire pipeline (or at least the Enterprise Manifest) to ensure integrity before merging.

**Action Items:**

- [x] **Convert `build_app_image`:** Rename `build_app_image_amd64.yml` -> `build_app_image.yml` and configure it to run for `amd64` and `arm64` using `cigen` architectures.
- [x] **Port `build_enterprise_image`:** Create `build_enterprise_image.yml` (multi-arch) in the `build` stage.
- [x] **Port `create_enterprise_docker_manifest`:** Create `create_enterprise_docker_manifest.yml` in the `build` stage (or a later stage if it needs to wait for both arch builds).
- [x] **Verify Manifest Jobs:** Ensure `create_app_docker_manifest` and `create_enterprise_docker_manifest` correctly depend on the respective `amd64` and `arm64` build jobs.
