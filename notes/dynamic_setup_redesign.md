# Dynamic Setup Redesign (Two‑File CircleCI Architecture)

This note summarizes the new direction for CIGen’s CircleCI dynamic setup, aligning with DocSpring’s production needs and replacing the previous per‑workflow split approach.

## Goals

- Use exactly two CircleCI configs:
  - `.circleci/config.yml` — entrypoint. Contains:
    - workflow `package_updates`: runs immediately when `pipeline.parameters.check_package_versions == true`.
    - workflow `staging_postman_tests`: runs immediately when `pipeline.parameters.run_staging_postman_tests == true`.
    - workflow `setup`: runs only when neither param is true; performs skip‑gating and posts a filtered continuation.
  - `.circleci/main.yml` — CI/CD workflow (tests/build/deploy). Designed for dynamic skip.
- No per‑job runtime skip on CircleCI when `dynamic=true`. Setup is the gate.
- Skip gating in setup uses CircleCI’s cache save/restore only (no Redis).
- Use a dedicated `docspringcom/cigen` runtime image (no Ruby dependency) for setup.
- On CI, ensure `.circleci/config.yml` is up‑to‑date (self‑check), optionally auto‑commit+push and fail if drift is detected (opt‑in).

## Behavior

1. Immediate workflows (in `.circleci/config.yml`)

- `package_updates` runs immediately if `check_package_versions` param is true.
- `staging_postman_tests` runs immediately if `run_staging_postman_tests` param is true.
- Otherwise, neither runs.

2. Dynamic setup workflow (in `.circleci/config.yml`)

- Image: `docspringcom/cigen:latest` (fallback curl installer for cigen until published).
- Steps:
  - `checkout`
  - `cigen generate` → writes `.circleci/main.yml` (and re‑renders `.circleci/config.yml` for self‑check).
  - Self‑check (opt‑in): verify current `.circleci/config.yml` matches what `cigen generate` would produce. If not:
    - Optionally `git add && git commit && git push` (opt‑in), then fail the build to force a new run on the updated entrypoint.
  - Skip analysis for `main` only:
    - For each job+arch in `main` with `source_files`:
      - Compute `JOB_HASH` in setup (same hashing logic as jobs).
      - Write a variant job‑key: `/tmp/setup_keys/<job_arch>/job-key` (`${JOB_NAME}-${DOCKER_ARCH}-${JOB_HASH}`).
      - `restore_cache` with key `job_status-exists-v1-{{ checksum "/tmp/setup_keys/<job_arch>/job-key" }}`
      - If `/tmp/cigen_job_exists/done_${JOB_HASH}` exists, append `<job_arch>` to `/tmp/skip/main.txt`.
      - Clear `/tmp/cigen_job_exists` before the next restore.
  - `cigen generate --workflow main` with `CIGEN_SKIP_JOBS_FILE=/tmp/skip/main.txt` to prune jobs and transitive dependents; write filtered continuation.
  - Continue with the filtered `.circleci/main.yml` via the Continuation API.

3. Jobs (CircleCI with dynamic=true)

- Do NOT inject per‑job runtime `restore_cache + halt`. Setup is the gate.
- Still record an “exists” marker at end of job:
  - touch `/tmp/cigen_job_exists/done_${JOB_HASH}`
  - `save_cache` with key `job_status-exists-v1-{{ checksum "/tmp/cigen_job_status/job-key" }}`; paths `/tmp/cigen_job_exists`
- Keep the job-status cache write if desired, but runtime halt is disabled for CircleCI dynamic.

## CIGen Implementation Outline

- Add `workflows_meta` (internal) to drive generation; but final shape is two files only:
  - `config.yml` — contains param‑guarded `package_updates` and `staging_postman_tests`, and a synthesized `setup` workflow.
  - `main.yml` — the CI/CD workflow.
- Generator changes:
  - Always emit `main.yml` for CircleCI dynamic.
  - Synthesize `config.yml` with three workflows as above (no split files for package updates or staging).
  - Disable per‑job runtime skip injection when `dynamic=true` and `provider=circleci`; still add exists‑cache save step at end of jobs with `source_files`.
  - Add `CIGEN_SKIP_JOBS_FILE` support for `cigen generate --workflow main` to prune jobs and transitive dependents (already implemented).
- Setup generation:
  - Uses `docspring/cigen:latest`; fallback curl installer in case the image isn’t on the runner yet.
  - Emits a deterministic loop of `restore_cache` probes (one per job variant in `main`) and collects a skip list.
  - Emits a self‑check step (opt‑in) that regenerates `config.yml` and verifies drift.

## Self‑Check (Opt‑In)

- Config key (to be added):

```yaml
setup:
  self_check:
    enabled: true
    commit_on_diff: true # optional: add+commit+push then fail
```

- If enabled, setup:
  - Regenerates `.circleci/config.yml` into a temp path.
  - If different from the existing entrypoint, optionally commit/push and fail the build.
  - This ensures the dynamic entrypoint is always up‑to‑date.

## Notes

- This design avoids per‑job runtime halts in CircleCI and does all skip‑gating up front, matching DocSpring’s Ruby implementation goals.
- “Immediate” workflows (package updates, staging postman) run right away via parameters; no hand‑off to setup.
- Scheduled workflows are handled by setting the parameter on the schedule.
- BASE_HASH/file hashing is anchored and cached at the file/content level and is millisecond‑fast.

## Next Steps

- Implement generation of combined `.circleci/config.yml` (package_updates + staging_postman_tests + setup) and `.circleci/main.yml` only.
- Emit the skip‑probe loop in setup with static `restore_cache` steps per job variant in `main`.
- Add setup self‑check (opt‑in) with optional auto‑commit+push.
- Publish `docspring/cigen` and switch setup to use it without fallback.
- Validate end‑to‑end on DocSpring: push branch, confirm pipeline behavior (immediate workflows via params, setup gating, minimal continuation).
