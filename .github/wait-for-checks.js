/**
 * Wait for GitHub Actions checks to complete on the current commit
 *
 * Tailored for the cigen repo: waits for CI and Docs checks.
 *
 * @param {Object} params
 * @param {Object} params.github - GitHub API object
 * @param {Object} params.context - GitHub context
 * @param {Object} params.core - GitHub Actions core
 * @param {Array<string>} params.checks - Optional array of check name prefixes to wait for
 */

const DEFAULT_REQUIRED_PREFIXES = [
  "Test", // CI job name from ci.yml
  "CI Gate", // first job in docs.yml
  "Build Docs", // docs build job
  "Deploy Docs", // docs deploy job
];

const SEC = 1000; // milliseconds in a second
/* biome-ignore lint/style/noMagicNumbers: configuring timeout in minutes */
const TIMEOUT_MS = 30 * 60 * SEC; // 30 minutes overall timeout
/* biome-ignore lint/style/noMagicNumbers: configuring warmup in minutes */
const WARMUP_MS = 5 * 60 * SEC; // 5 minutes for checks to appear
const POLL_INTERVAL_MS = 10 * SEC; // 10 seconds between polls
/* biome-ignore lint/style/noMagicNumbers: configuring retry sleep seconds */
const RETRY_SLEEP_MS = 5 * SEC; // retry interval during warmup

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function listChecks(github, context) {
  const { data } = await github.rest.checks.listForRef({
    owner: context.repo.owner,
    repo: context.repo.repo,
    ref: context.sha,
    per_page: 100,
  });
  return data.check_runs.map((c) => ({
    name: c.name,
    status: c.status, // queued, in_progress, completed
    conclusion: c.conclusion, // success, failure, neutral, cancelled, etc
  }));
}

function makeMatcher(requiredPrefixes) {
  return (name) => requiredPrefixes.some((prefix) => name.startsWith(prefix));
}

async function discoverPresentChecks({
  github,
  context,
  core,
  requiredPrefixes,
}) {
  core.info("Discovering present checks...");
  const matchesRequired = makeMatcher(requiredPrefixes);
  let presentChecks = [];
  const warmupStart = Date.now();

  while (Date.now() - warmupStart < WARMUP_MS) {
    const listedChecks = await listChecks(github, context);
    presentChecks = listedChecks.filter((c) => matchesRequired(c.name));

    if (presentChecks.length > 0) {
      core.info(
        `Found ${presentChecks.length} required check(s): ${presentChecks
          .map((c) => c.name)
          .join(", ")}`
      );
      break;
    }

    core.info(
      `No required checks found yet, waiting... (${Math.round(
        (Date.now() - warmupStart) / SEC
      )}s elapsed)`
    );
    await sleep(RETRY_SLEEP_MS);
  }

  return presentChecks;
}

async function waitForRequiredChecks({
  github,
  context,
  core,
  requiredPrefixes,
  presentChecks,
}) {
  const matchesRequired = makeMatcher(requiredPrefixes);
  core.info(`Waiting for ${presentChecks.length} check(s) to complete...`);
  const waitStart = Date.now();

  while (Date.now() - waitStart < TIMEOUT_MS) {
    const listedChecks = await listChecks(github, context);
    const relevant = listedChecks.filter((c) => matchesRequired(c.name));

    // If checks disappeared (canceled?), keep waiting within timeout
    if (relevant.length === 0) {
      core.warning("Required checks disappeared - they may have been canceled");
      await sleep(POLL_INTERVAL_MS);
      continue;
    }

    const pending = relevant.filter((c) => c.status !== "completed");

    if (pending.length === 0) {
      // All checks completed - verify conclusions
      const failed = relevant.filter(
        (c) => c.conclusion !== "success" && c.conclusion !== "skipped"
      );

      if (failed.length > 0) {
        const failureDetails = failed
          .map((f) => `${f.name} (${f.conclusion})`)
          .join(", ");
        core.setFailed(`Some required checks failed: ${failureDetails}`);
        process.exit(1);
      }

      const successful = relevant.filter((c) => c.conclusion === "success");
      core.info(
        `✅ All ${successful.length} required check(s) passed successfully!`
      );
      return;
    }

    const elapsed = Math.round((Date.now() - waitStart) / SEC);
    const pendingDetails = pending
      .map((p) => `${p.name} (${p.status})`)
      .join(", ");
    core.info(`[${elapsed}s] Waiting for: ${pendingDetails}`);

    await sleep(POLL_INTERVAL_MS);
  }

  core.setFailed(
    `Timeout after ${Math.round(TIMEOUT_MS / SEC)}s waiting for checks to complete`
  );
  process.exit(1);
}

module.exports = async ({ github, context, core, checks }) => {
  const REQUIRED_PREFIXES = checks?.length ? checks : DEFAULT_REQUIRED_PREFIXES;

  if (checks?.length) {
    core.info(`Waiting for specific checks: ${checks.join(", ")}`);
  } else {
    core.info("Waiting for default required checks (CI + Docs)");
  }

  const presentChecks = await discoverPresentChecks({
    github,
    context,
    core,
    requiredPrefixes: REQUIRED_PREFIXES,
  });

  if (presentChecks.length === 0) {
    core.info(
      "No required checks present on this commit - continuing without waiting."
    );
    core.info("This can happen if workflows were skipped by path filters.");
    return;
  }

  await waitForRequiredChecks({
    github,
    context,
    core,
    requiredPrefixes: REQUIRED_PREFIXES,
    presentChecks,
  });
};
