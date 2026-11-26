#!/bin/bash
# Tests scripts/ci/flaky-weekly.sh against a stand-in for gh that answers with the shape of GitHub's API.
# shellcheck disable=SC2016  # the backticks are Markdown, in text that is compared as it is
set -euo pipefail
cd "$(dirname "$0")/../.."
fakes="$PWD/scripts/test/flaky-fakes"
script="$PWD/scripts/ci/flaky-weekly.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }
expect() { [ "$2" = "$3" ] || fail "$1: got '$2', wanted '$3'"; }

FAKE_STATE="$work"; export FAKE_STATE
run() { set +e; output="$(PATH="$fakes:$PATH" "$script" "$@" 2>&1)"; status=$?; set -e; }
annotation() { printf '{"title":"Flaky test","message":"%s","annotation_level":"warning"}' "$1"; }

# Three runs: two on pull request branches, one with nothing. Jobs: rust, web, e2e, and another.
cat > "$work/runs.json" <<'JSON'
{"total_count":3,"workflow_runs":[
 {"id":11,"head_branch":"feat/a","html_url":"https://example.test/runs/11"},
 {"id":12,"head_branch":"fix/b","html_url":"https://example.test/runs/12"},
 {"id":13,"head_branch":"main","html_url":"https://example.test/runs/13"}]}
JSON
for id in 11 12 13; do
  cat > "$work/jobs-$id.json" <<JSON
{"jobs":[{"id":${id}1,"name":"rust"},{"id":${id}2,"name":"web"},{"id":${id}3,"name":"e2e"},{"id":${id}4,"name":"words"}]}
JSON
done
echo "[$(annotation 'rust: alpha::first'),{\"title\":\"something else\",\"message\":\"x\"}]" > "$work/annotations-111.json"
echo "[$(annotation 'web: src/a.test.tsx > suite > it')]" > "$work/annotations-112.json"
# The same test twice in one run (two attempts): it is counted twice, and its branch is listed once.
echo "[$(annotation 'rust: alpha::first'),$(annotation 'rust: alpha::first')]" > "$work/annotations-121.json"
echo "[$(annotation 'e2e: chromium > desk.spec.ts > opens')]" > "$work/annotations-123.json"
# A job that is not one of the three is not read, even when it has such an annotation.
echo "[$(annotation 'rust: never-read')]" > "$work/annotations-114.json"

run acme/app 265 --dry-run
expect "a dry run exits 0" "$status" 0
case "$output" in *"## Tests that passed only on a retry: "*) ;; *) fail "no heading: $output" ;; esac
expect "each time a test was flaky is counted" "$(printf '%s\n' "$output" | grep -c '^| `rust: alpha::first` | 3 |')" 1
case "$output" in *'| `rust: alpha::first` | 3 | feat/a, fix/b | the node team |'*) ;; *) fail "the rust test is not listed with its branches and owner: $output" ;; esac
case "$output" in *'| `web: src/a.test.tsx > suite > it` | 1 | feat/a | the client team (web) |'*) ;; *) fail "the web test is not listed: $output" ;; esac
case "$output" in *'| `e2e: chromium > desk.spec.ts > opens` | 1 | fix/b | the client team (web) |'*) ;; *) fail "the e2e test is not listed: $output" ;; esac
case "$output" in *"never-read"*) fail "a job that is not rust, web, or e2e was read" ;; esac
case "$output" in *'`x`'*) fail "an annotation that is not a flaky test was listed" ;; esac
expect "the most frequent test comes first" "$(printf '%s\n' "$output" | grep -m1 '^| `' | cut -d'|' -f2)" ' `rust: alpha::first` '
[ ! -e "$work/posted.md" ] || fail "a dry run posted a comment"
case "$(cat "$work/runs-asked")" in *"created=>="*"per_page=100"*"--paginate"*) ;; *) fail "the runs were not asked for by date, all pages: $(cat "$work/runs-asked")" ;; esac

run acme/app 265
expect "without --dry-run it exits 0" "$status" 0
[ -f "$work/posted.md" ] || fail "no comment was posted"
case "$(cat "$work/posted.md")" in *'`rust: alpha::first` | 3'*) ;; *) fail "the posted comment is not the list" ;; esac

rm -f "$work"/annotations-*.json "$work/posted.md"
run acme/app 265
case "$(cat "$work/posted.md")" in *"None this week"*) ;; *) fail "a quiet week is not said to be quiet: $(cat "$work/posted.md")" ;; esac

run acme/app
expect "a missing issue number is a usage error" "$status" 2
run acme/app 265 --days x
expect "a days that is not a number is a usage error" "$status" 2
run app 265
expect "a repository that is not owner/name is a usage error" "$status" 2

[ "$failures" -eq 0 ] && echo "flaky-weekly: ok"
exit "$failures"
