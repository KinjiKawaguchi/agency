#!/bin/bash
# Tests scripts/ci/cargo-test-retry.sh and scripts/ci/flaky-summary.sh, against a stand-in for cargo.
set -euo pipefail
cd "$(dirname "$0")/../.."
fakes="$PWD/scripts/test/ci-fakes"
retry_script="$PWD/scripts/ci/cargo-test-retry.sh"
summary_script="$PWD/scripts/ci/flaky-summary.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }
expect() { [ "$2" = "$3" ] || fail "$1: got '$2', wanted '$3'"; }

# run <mode> [cargo test arguments]: sets status, and leaves the calls and the report in $work/state.
run() {
  rm -rf "$work/state"; mkdir -p "$work/state"
  FAKE_STATE="$work/state" FAKE_MODE="$1"; export FAKE_STATE FAKE_MODE
  shift
  set +e
  output="$(PATH="$fakes:$PATH" FLAKY_REPORT="$work/state/report" "$retry_script" "$@" 2>&1)"
  status=$?
  set -e
}
calls() { wc -l < "$work/state/calls" | tr -d ' '; }
report() { cat "$work/state/report" 2>/dev/null || true; }

run ok
expect "tests that pass need no retry and exit 0" "$status" 0
expect "cargo test runs once" "$(calls)" 1
expect "and records nothing" "$(report)" ""
expect "it runs every test of the workspace by default, without stopping at the first failure" "$(head -n 1 "$work/state/calls")" "test --no-fail-fast --workspace --locked"

run flaky
expect "tests that fail and then pass exit 0" "$status" 0
expect "cargo test runs twice" "$(calls)" 2
expect "the second run is of the failed tests only, and exactly" "$(sed -n 2p "$work/state/calls")" "test --no-fail-fast --workspace --locked -- --exact alpha::first beta::second"
expect "both are recorded as flaky" "$(report)" "$(printf 'rust: alpha::first\nrust: beta::second')"
case "$output" in *"passed on the retry"*) ;; *) fail "the output does not say they passed on the retry: $output" ;; esac

run fail
expect "tests that fail twice fail the run" "$status" 1
expect "cargo test runs twice" "$(calls)" 2
expect "and nothing is recorded as flaky" "$(report)" ""

run build
expect "a failure with no failing test fails at once, with cargo's status" "$status" 101
expect "without a retry" "$(calls)" 1
case "$output" in *"nothing to retry"*) ;; *) fail "the output does not say there was nothing to retry: $output" ;; esac

run flaky -p agency-node --locked
expect "cargo test arguments are passed on, to both runs" "$(sed -n 2p "$work/state/calls")" "test --no-fail-fast -p agency-node --locked -- --exact alpha::first beta::second"

# The summary.
summarize() { # summarize <report files...>; the summary goes to $work/summary, the rest to $output
  rm -f "$work/summary"
  output="$(GITHUB_STEP_SUMMARY="$work/summary" "$summary_script" "$@" 2>&1)"
}
printf 'rust: alpha::first\nweb: src/a.test.ts > suite > it\n' > "$work/one"
printf 'rust: alpha::first\ne2e: spec > title\n\n' > "$work/two"
summarize "$work/one" "$work/two" "$work/missing"
expect "each flaky test is listed once, by its runner" "$(grep -c '^- ' "$work/summary")" 3
# shellcheck disable=SC2016  # the backticks are for Markdown
case "$(cat "$work/summary")" in *'- `e2e: spec > title`'*'- `rust: alpha::first`'*'- `web: src/a.test.ts > suite > it`'*) ;; *) fail "the summary does not list them: $(cat "$work/summary")" ;; esac
expect "and each is a warning on the run" "$(printf '%s\n' "$output" | grep -c '^::warning title=Flaky test::')" 3
summarize "$work/missing"
case "$(cat "$work/summary")" in *"None: no test needed a retry."*) ;; *) fail "an empty run says nothing: $(cat "$work/summary")" ;; esac
expect "and gives no warning" "$(printf '%s' "$output" | grep -c '::warning')" 0

[ "$failures" -eq 0 ] && echo "ci-scripts: ok"
exit "$failures"
