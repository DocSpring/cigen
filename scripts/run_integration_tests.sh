#!/bin/sh
# POSIX-compliant integration test runner for cigen
# Exits non-zero on any test failure.

set -eu

# Resolve repo root (directory containing this script)
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

# Ensure cigen builds first (use existing toolchain)
cd "$REPO_ROOT"
if ! cargo build >/dev/null 2>&1; then
  echo "ERROR: cargo build failed" >&2
  exit 1
fi

# Helper: generate config for a fixture directory
# Arguments: <fixture_path>
generate_for_fixture() {
  FIXTURE_DIR=$1
  if [ ! -d "$FIXTURE_DIR/.cigen" ]; then
    echo "ERROR: Missing .cigen directory in fixture: $FIXTURE_DIR" >&2
    return 1
  fi
  OUTPUT_DIR="$FIXTURE_DIR/.circleci"
  rm -rf "$OUTPUT_DIR"
  mkdir -p "$OUTPUT_DIR"
  (
    cd "$FIXTURE_DIR" && \
    CIGEN_SKIP_CIRCLECI_CLI=1 cargo run -q --manifest-path "$REPO_ROOT/Cargo.toml" -- --config .cigen generate
  )
}

# Test 1: split vs inline produce identical outputs
TEST1_SPLIT="$REPO_ROOT/integration_tests/circleci_node_simple_split"
TEST1_INLINE="$REPO_ROOT/integration_tests/circleci_node_simple_inline"

# Generate both
generate_for_fixture "$TEST1_SPLIT"
generate_for_fixture "$TEST1_INLINE"

# Compare config files
CONFIG_A="$TEST1_SPLIT/.circleci/config.yml"
CONFIG_B="$TEST1_INLINE/.circleci/config.yml"
if [ ! -f "$CONFIG_A" ] || [ ! -f "$CONFIG_B" ]; then
  echo "ERROR: Missing generated config.yml for split/inline fixtures" >&2
  exit 1
fi

if ! diff -u "$CONFIG_A" "$CONFIG_B" >/dev/null 2>&1; then
  echo "ERROR: split vs inline config differ" >&2
  echo "--- diff ---"
  diff -u "$CONFIG_A" "$CONFIG_B" || true
  exit 1
fi

echo "✓ Integration tests passed"
