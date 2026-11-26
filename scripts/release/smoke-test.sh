#!/bin/bash
# Starts a release build of the node on a throwaway data directory and a port in 4700 to 4799, and checks
# that it answers its API and serves the web client. usage: scripts/release/smoke-test.sh <agency binary>
set -euo pipefail

agency="${1:?usage: $0 <agency binary>}"
port="${SMOKE_PORT:-4790}"
work="$(mktemp -d)"
node_pid=""
cleanup() {
  [ -z "$node_pid" ] || kill "$node_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

"$agency" --help >/dev/null
"$agency" --dir "$work" init --account ci --label CI \
  --url "http://127.0.0.1:$port" --listen "127.0.0.1:$port" --scrum true >/dev/null
"$agency" --dir "$work" serve >"$work/serve.log" 2>&1 &
node_pid=$!

for _ in $(seq 1 50); do
  curl -sf "http://127.0.0.1:$port/api/me" >"$work/me.json" 2>/dev/null && break
  kill -0 "$node_pid" 2>/dev/null || { echo "the node stopped:" >&2; cat "$work/serve.log" >&2; exit 1; }
  sleep 0.2
done
grep -q '"id":"ci"' "$work/me.json" || { echo "/api/me does not list the account:" >&2; cat "$work/me.json" >&2; exit 1; }
page="$(curl -sf "http://127.0.0.1:$port/")"
case "$page" in *"<html"*) ;; *) echo "the web client is not served (was it built before the node?)" >&2; exit 1 ;; esac
echo "smoke test: ok"
