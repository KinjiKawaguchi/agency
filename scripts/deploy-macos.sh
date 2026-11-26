#!/bin/bash
# Deploys a node on macOS: builds it, swaps the installed binary in one step, restarts its launchd job,
# checks that the node answers, and puts the previous binary back if it does not.
#
#   scripts/deploy-macos.sh --agency <installed binary> --dir <data directory> --label <launchd label> [options]
#
#   --binary <file>     deploy this built binary instead of building one
#   --repo <directory>  the repository to build from (default: the one this script is in)
#   --url <url>         where the node answers (default: the `listen` address in <data directory>/config.json)
#   --timeout <seconds> how long the restarted node has to answer (default 60)
#   --settle <seconds>  how long it must then keep running (default 5), so that a node that starts and
#                       stops again is not taken for a good one
#   --allow-dirty       build from a working tree with uncommitted changes (refused otherwise)
#   --skip-preflight    do not start the new binary on a throwaway data directory first
#   --rollback          swap the installed binary and the previous one, then restart and check, instead of building
#
# The previous binary stays next to the installed one as <installed binary>.previous. The data directory
# and the launchd job's file are not touched; the job must already be loaded (docs/install.md).
# Exit status: 0 deployed, 1 not deployed (the previous binary is back and answering), 2 not deployed and
# the previous binary does not answer either (look at the node now).
set -euo pipefail

usage() {
  sed -n '2,/^set -euo/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//' >&2
  exit 2
}

agency="" dir="" label="" binary="" url=""
repo="$(cd "$(dirname "$0")/.." && pwd -P)"
timeout=60 settle=5 allow_dirty=0 preflight=1 rollback=0
while [ $# -gt 0 ]; do
  case "$1" in
    --agency) agency="${2:-}"; shift 2 ;;
    --dir) dir="${2:-}"; shift 2 ;;
    --label) label="${2:-}"; shift 2 ;;
    --binary) binary="${2:-}"; shift 2 ;;
    --repo) repo="${2:-}"; shift 2 ;;
    --url) url="${2:-}"; shift 2 ;;
    --timeout) timeout="${2:-}"; shift 2 ;;
    --settle) settle="${2:-}"; shift 2 ;;
    --allow-dirty) allow_dirty=1; shift ;;
    --skip-preflight) preflight=0; shift ;;
    --rollback) rollback=1; shift ;;
    *) usage ;;
  esac
done
[ -n "$agency" ] && [ -n "$dir" ] && [ -n "$label" ] || usage
case "$timeout$settle" in *[!0-9]*) echo "--timeout and --settle are whole numbers of seconds" >&2; exit 2 ;; esac

say() { echo "deploy: $*"; }
fail() { echo "deploy: $*" >&2; exit 1; }

[ -f "$dir/config.json" ] || fail "$dir has no config.json"
agency="$(cd "$(dirname "$agency")" 2>/dev/null && pwd -P)/$(basename "$agency")" || fail "the directory of $agency does not exist"
if [ -z "$url" ]; then
  listen="$(sed -n 's/.*"listen"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$dir/config.json" | head -n 1)"
  [ -n "$listen" ] || fail "cannot read \"listen\" from $dir/config.json; give --url"
  url="http://$listen"
fi
target="gui/$(id -u)/$label"
previous="$agency.previous"

# Only one deploy at a time: two of them would swap binaries under each other.
lock="$agency.deploy-lock"
mkdir "$lock" 2>/dev/null || fail "another deploy holds $lock (remove it if none is running)"
scratch=""
trap 'rm -rf "$lock" "$scratch"' EXIT
scratch="$(mktemp -d "$agency.deploy.XXXXXX")"

job_pid() { launchctl print "$target" 2>/dev/null | sed -n 's/^[[:space:]]*pid = \([0-9][0-9]*\)$/\1/p' | head -n 1; }
answers() { curl -sf --max-time 3 -o /dev/null "$url/api/me"; }

# Restarts the job and waits for the node to answer and to keep running. Returns 1 with the reason on
# standard error when it does not.
restart_and_check() {
  local before pid after waited=0
  before="$(job_pid || true)"
  launchctl kickstart -k "$target" || { echo "launchctl could not restart $label" >&2; return 1; }
  while [ "$waited" -lt "$timeout" ]; do
    pid="$(job_pid || true)"
    if [ -n "$pid" ] && [ "$pid" != "$before" ] && answers; then
      sleep "$settle"
      after="$(job_pid || true)"
      [ "$after" = "$pid" ] && answers && return 0
      echo "the node answered, then stopped or restarted again" >&2
      return 1
    fi
    sleep 1
    waited=$((waited + 1))
  done
  echo "the node did not answer at $url within $timeout seconds" >&2
  return 1
}

log_tail() {
  [ -f "$dir/logs/serve.log" ] || return 0
  echo "the end of $dir/logs/serve.log:" >&2
  tail -n 15 "$dir/logs/serve.log" | sed 's/^/    /' >&2
}

# Puts the file at $1 in place as the installed binary, in one rename, and keeps the one it replaces as
# the previous binary. A running node keeps the file it started from.
install_binary() {
  cp "$1" "$scratch/new" && chmod 755 "$scratch/new"
  if [ -f "$agency" ]; then cp -p "$agency" "$scratch/old"; fi
  cp "$scratch/new" "$agency.new.$$" && mv -f "$agency.new.$$" "$agency"
  if [ -f "$scratch/old" ]; then cp -p "$scratch/old" "$previous.new.$$" && mv -f "$previous.new.$$" "$previous"; fi
}

launchctl print "$target" >/dev/null 2>&1 || fail "$label is not loaded in $target; load it first (docs/install.md, \"Run it with launchd\")"

if [ "$rollback" -eq 1 ]; then
  [ -f "$previous" ] || fail "there is no previous binary ($previous)"
  new="$previous"
  say "swapping $agency and $previous"
else
  if [ -z "$binary" ]; then
    cd "$repo"
    if [ "$allow_dirty" -eq 0 ] && [ -n "$(git status --porcelain)" ]; then
      fail "$repo has uncommitted changes; commit them or use --allow-dirty"
    fi
    say "building $(git describe --always --dirty) from $repo"
    (cd web && npm ci && npm run build)
    cargo build --release --locked -p agency-node
    binary="$repo/target/release/agency"
  fi
  [ -x "$binary" ] || fail "$binary is not an executable file"
  "$binary" --help >/dev/null || fail "$binary does not run on this machine"
  if [ "$preflight" -eq 1 ] && [ -x "$repo/scripts/release/smoke-test.sh" ]; then
    say "checking that the new binary serves, on a throwaway data directory"
    "$repo/scripts/release/smoke-test.sh" "$binary" >/dev/null || fail "the new binary does not start and serve; the node is untouched"
  fi
  new="$binary"
fi

had_binary=0
[ -f "$agency" ] && had_binary=1
[ "$had_binary" -eq 1 ] && cp -p "$agency" "$scratch/before"
install_binary "$new"
say "installed $agency; restarting $label"
if restart_and_check; then
  say "the node answers at $url; the previous binary is $previous"
  exit 0
fi
log_tail

if [ "$rollback" -eq 1 ] || [ "$had_binary" -eq 0 ]; then
  [ "$had_binary" -eq 1 ] || echo "deploy: there was no binary before this one to go back to" >&2
  exit 1
fi
say "putting the previous binary back"
cp "$scratch/before" "$scratch/restore" && mv -f "$scratch/restore" "$agency.restore.$$" && mv -f "$agency.restore.$$" "$agency"
if restart_and_check; then
  say "the previous binary is back and the node answers at $url"
  exit 1
fi
echo "deploy: the previous binary does not answer either; look at the node now ($target)" >&2
exit 2
