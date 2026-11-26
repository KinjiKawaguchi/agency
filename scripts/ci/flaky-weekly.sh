#!/bin/bash
# Posts the week's list of tests that passed only on a retry as a comment on a standing issue, so that
# retries do not hide flakes for good. The list is read from the warnings that CI puts on its runs
# (`Flaky test`, see scripts/ci/flaky-summary.sh): every run of the `ci` workflow of the last days, and the
# annotations of its rust, web, and e2e jobs.
#
#   scripts/ci/flaky-weekly.sh <owner/name> <issue number> [--days 7] [--dry-run]
#
# --dry-run prints the comment instead of posting it. Needs `gh` (logged in, or GH_TOKEN) and `jq`.
set -euo pipefail

usage() { echo "usage: $0 <owner/name> <issue number> [--days n] [--dry-run]" >&2; exit 2; }
[ $# -ge 2 ] || usage
repo="$1" issue="$2"; shift 2
days=7 dry=0
while [ $# -gt 0 ]; do
  case "$1" in
    --days) days="${2:-}"; shift 2 ;;
    --dry-run) dry=1; shift ;;
    *) usage ;;
  esac
done
case "$issue$days" in *[!0-9]*) usage ;; esac
case "$repo" in */*) ;; *) usage ;; esac

since="$(date -u -d "$days days ago" +%F 2>/dev/null || date -u -v-"${days}"d +%F)"
today="$(date -u +%F)"

# Who a flaky test is handed to, from the runner that reported it. These are roles, not people.
owner_of() {
  case "$1" in
    rust:*) echo "the node team" ;;
    web:* | e2e:*) echo "the client team (web)" ;;
    *) echo "the lead" ;;
  esac
}

runs="$(gh api "repos/$repo/actions/workflows/ci.yml/runs" --method GET -f "created=>=$since" -f per_page=100 --paginate \
  | jq -r '.workflow_runs[] | [.id, .head_branch, .html_url] | @tsv')"

rows="$(mktemp)"
trap 'rm -f "$rows"' EXIT
while IFS=$'\t' read -r id branch url; do
  [ -n "$id" ] || continue
  jobs="$(gh api "repos/$repo/actions/runs/$id/jobs" --paginate | jq -r '.jobs[] | select(.name == "rust" or .name == "web" or .name == "e2e") | .id')"
  for job in $jobs; do
    gh api "repos/$repo/check-runs/$job/annotations" --paginate \
      | jq -r --arg branch "$branch" --arg url "$url" '.[] | select(.title == "Flaky test") | [.message, $branch, $url] | @tsv' >> "$rows"
  done
done <<< "$runs"

body="$(mktemp)"
trap 'rm -f "$rows" "$body"' EXIT
{
  echo "## Tests that passed only on a retry: $since to $today"
  echo
  if [ ! -s "$rows" ]; then
    echo "None this week: no test needed a retry in a run of CI."
  else
    echo "Each failed and then passed within a run of CI, so CI was green. Each is a flake to fix, or to hand to the owner of its code, this week."
    echo
    echo "| Test | Times | In | Handed to |"
    echo "|---|---|---|---|"
    cut -f1 "$rows" | sort | uniq -c | sort -rn | while read -r times test; do
      where="$(awk -F'\t' -v t="$test" '$1 == t { print $2 }' "$rows" | sort -u | head -n 3 | paste -sd',' - | sed 's/,/, /g')"
      echo "| \`$test\` | $times | $where | $(owner_of "$test") |"
    done
    echo
    echo "Reply to this comment with the pull request that fixes each one."
  fi
} > "$body"

if [ "$dry" -eq 1 ]; then
  cat "$body"
else
  gh issue comment "$issue" --repo "$repo" --body-file "$body"
fi
