#!/bin/bash
# Runs the web client's tests as if it were another day, for a handful of days (each of the next seven, the end of the month and
# of the year, and the next change of the clock of a few time zones: see web/src/test/shiftedDates.ts), and posts the days that
# broke a test as a comment on a standing issue, as the weekly list of flaky tests is. A test that is true only on the day it was
# written (a number made up from the date, a week that has moved on) is found here before it turns the next PR red.
#
#   scripts/ci/shifted-clock.sh <owner/name> <issue number> [--dry-run]
#   scripts/ci/shifted-clock.sh --at <moment>[@<time zone>]        one run, nothing posted: for finding a day locally
#
# Examples: `--at 2026-11-28T12:00:00Z`, `--at 2027-03-14T06:50:00Z@America/New_York`. The same one run without the script is
# `cd web && AGENCY_TEST_NOW=2026-11-28T12:00:00Z TZ=Asia/Tokyo npm test`. --dry-run prints the comment instead of posting it.
# Needs node, the web client's dependencies installed, and `gh` (logged in, or GH_TOKEN) to post.
set -euo pipefail

usage() { echo "usage: $0 <owner/name> <issue number> [--dry-run] | $0 --at <moment>[@<time zone>]" >&2; exit 2; }

repo="" issue="" dry=0 at=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) dry=1; shift ;;
    --at) at="${2:-}"; [ -n "$at" ] || usage; shift 2 ;;
    -*) usage ;;
    *) if [ -z "$repo" ]; then repo="$1"; elif [ -z "$issue" ]; then issue="$1"; else usage; fi; shift ;;
  esac
done
if [ -z "$at" ]; then
  [ -n "$repo" ] && [ -n "$issue" ] || usage
  case "$issue" in *[!0-9]*) usage ;; esac
  case "$repo" in */*) ;; *) usage ;; esac
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root/web"

if [ -n "$at" ]; then
  now="${at%%@*}" zone="UTC"
  case "$at" in *@*) zone="${at#*@}" ;; esac
  runs="$(printf 'a moment you gave\t%s\t%s\n' "$now" "$zone")"
else
  runs="$(node scripts/shifted-dates.ts)"
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
broken="$work/broken.txt"
: > "$broken"

while IFS=$'\t' read -r name now zone; do
  [ -n "$name" ] || continue
  log="$work/run.log"
  echo "== $name: the clock starts at $now, in $zone"
  # No retries: a test that depends on the day fails on every try, and a flake has its own list.
  if env -u CI AGENCY_TEST_NOW="$now" TZ="$zone" npx vitest run > "$log" 2>&1; then
    grep -E '^ *Tests ' "$log" || true
    continue
  fi
  failed="$(grep -E '^ *(×|FAIL) ' "$log" | sed -E 's/^ *//; s/ [0-9]+ms$//' | sort -u | head -20 || true)"
  [ -n "$failed" ] || failed="the run did not finish: $(tail -n 3 "$log" | tr '\n' ' ')"
  {
    # The backticks are Markdown, meant as they are.
    # shellcheck disable=SC2016
    printf -- '- **%s**: the clock starts at `%s`, in `%s`. Run it: `cd web && AGENCY_TEST_NOW=%s TZ=%s npm test`\n' "$name" "$now" "$zone" "$now" "$zone"
    # shellcheck disable=SC2016
    printf '%s\n' "$failed" | sed 's/^/    - `/; s/$/`/'
  } >> "$broken"
done <<< "$runs"

if [ ! -s "$broken" ]; then
  echo "No test broke on any of the days."
  exit 0
fi

body="$work/body.md"
{
  echo "## Tests that broke when the clock was moved: $(date -u +%F)"
  echo
  echo "Each of these days made a test fail that passes today, so something in it says a thing that is true only on the day it was written: a number made up from the date, a week that has moved on, an hour that is 23 or 25 hours long. Fix it by pinning the clock in the test (a node made with \`now\`), not by changing the page."
  echo
  cat "$broken"
} > "$body"

if [ -n "$at" ] || [ "$dry" = 1 ]; then
  cat "$body"
else
  gh issue comment "$issue" --repo "$repo" --body-file "$body"
fi
exit 1
