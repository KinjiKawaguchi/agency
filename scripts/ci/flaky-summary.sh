#!/bin/bash
# Shows the tests that failed and then passed on a retry, from the report files the test runners write
# (one line each: `<runner>: <test>`), in the summary of the CI run and as a warning on the run.
#
#   scripts/ci/flaky-summary.sh <report file>...
#
# A report file that is not there means the runner did not run, or nothing needed a retry.
set -euo pipefail

lines="$(for report in "$@"; do [ -f "$report" ] && cat "$report"; done | sed '/^$/d' | sort -u || true)"
summary="${GITHUB_STEP_SUMMARY:-/dev/stdout}"
{
  echo "### Tests that passed only on a retry"
  if [ -z "$lines" ]; then
    echo
    echo "None: no test needed a retry."
  else
    echo
    echo "These failed once and then passed, so CI is green, but each is a flake to fix (or to hand to its owner):"
    echo
    # shellcheck disable=SC2016  # the backticks are for Markdown
    printf '%s\n' "$lines" | sed 's/^/- `/; s/$/`/'
  fi
} >> "$summary"
if [ -n "$lines" ]; then
  # A warning on the run for each, so that it shows beside the pull request's checks.
  printf '%s\n' "$lines" | while IFS= read -r line; do
    echo "::warning title=Flaky test::$line"
  done
fi
