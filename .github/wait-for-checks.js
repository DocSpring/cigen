const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const DEFAULT_MAX_ATTEMPTS = 60;
const DEFAULT_INTERVAL_MS = 10_000;
const MILLISECONDS_IN_SECOND = 1000;

module.exports = async function waitForChecks({
  github,
  context,
  core,
  checks,
}) {
  const owner = context.repo.owner;
  const repo = context.repo.repo;
  const ref = context.sha;
  const desiredChecks =
    Array.isArray(checks) && checks.length > 0 ? checks : ["test"];
  const checkSet = new Set(desiredChecks);

  const maxAttempts = Number.parseInt(
    process.env.CIGEN_WAIT_CHECKS_ATTEMPTS || String(DEFAULT_MAX_ATTEMPTS),
    10
  );
  const intervalMs = Number.parseInt(
    process.env.CIGEN_WAIT_CHECKS_INTERVAL_MS || String(DEFAULT_INTERVAL_MS),
    10
  );

  core.info(
    `Waiting for checks [${desiredChecks.join(", ")}] on ${owner}/${repo}@${ref}`
  );

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    const { data } = await github.rest.checks.listForRef({
      owner,
      repo,
      ref,
    });

    const runs = data.check_runs.filter((run) => checkSet.has(run.name));

    if (runs.length === checkSet.size) {
      const incomplete = runs.filter((run) => run.status !== "completed");
      if (incomplete.length === 0) {
        const failures = runs.filter((run) => run.conclusion !== "success");
        if (failures.length > 0) {
          const summary = failures
            .map((run) => `${run.name} -> ${run.conclusion}`)
            .join(", ");
          throw new Error(`Checks failed: ${summary}`);
        }

        core.info("All required checks completed successfully.");
        return;
      }
    }

    core.info(
      `Attempt ${attempt}/${maxAttempts}: checks not complete yet. Sleeping ${(
        intervalMs / MILLISECONDS_IN_SECOND
      ).toFixed(1)}s...`
    );
    await sleep(intervalMs);
  }

  throw new Error(
    `Timed out waiting for checks: ${desiredChecks.join(", ")}. Increase CIGEN_WAIT_CHECKS_ATTEMPTS?`
  );
};
