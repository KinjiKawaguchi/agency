#!/bin/bash
# Runs `cargo test` and, when tests fail, runs only the tests that failed once more. A test that passes the
# second time is recorded in $FLAKY_REPORT (default flaky-tests.txt) as `rust: <name>`, and the run is
# green; a test that fails twice fails the run. A failure with no failing test (the build, a harness)
# fails at once.
#
#   scripts/ci/cargo-test-retry.sh [cargo test arguments]      (default: --workspace --locked)
set -uo pipefail

report="${FLAKY_REPORT:-flaky-tests.txt}"
args=("$@")
[ ${#args[@]} -gt 0 ] || args=(--workspace --locked)

output="$(mktemp)"
trap 'rm -f "$output"' EXIT
cargo test --no-fail-fast "${args[@]}" 2>&1 | tee "$output"
status=${PIPESTATUS[0]}
[ "$status" -ne 0 ] || exit 0

failed=()
while IFS= read -r name; do failed+=("$name"); done < <(sed -n 's/^test \(.*\) \.\.\. FAILED$/\1/p' "$output" | sort -u)
if [ ${#failed[@]} -eq 0 ]; then
  echo "cargo test failed with no failing test; nothing to retry" >&2
  exit "$status"
fi

echo "Retrying the ${#failed[@]} test(s) that failed: ${failed[*]}" >&2
if cargo test --no-fail-fast "${args[@]}" -- --exact "${failed[@]}"; then
  for name in "${failed[@]}"; do echo "rust: $name" >> "$report"; done
  echo "The tests passed on the retry; recorded in $report" >&2
  exit 0
fi
exit 1
