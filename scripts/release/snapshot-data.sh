#!/bin/bash
# Keeps what a release wrote, so that every later build is tested on reading it:
#
#   testdata/releases/<tag>/client/  the Client API's answers of the tag (its testdata/client/), which the
#                                    apps' and the web client's tests decode;
#   testdata/releases/<tag>/data/    the data directories of two nodes of the tag
#                                    (crates/agency-node/tests/earlier_releases.rs).
#
# A part that is kept already is left as it is, and said. The data comes from building the tag in a
# temporary worktree and running two nodes of that build, on empty data directories, through a fixed sequence:
#
#   alice makes the org acme, adds the team app (run by fixture-runtime.sh), adds her CLI device, serves;
#   bob joins acme, adds his CLI device, reads alice's team once, which leaves a replica;
#   alice answers the team's request with her CLI device, which writes the decision log.
#
# The two data directories are written to data/alice and data/bob, without the log
# and with local paths in config.json made fixed ones. The keys in them are test keys made by this run:
# the root key of acme is kept outside a data directory and is not written. usage:
#
#   scripts/release/snapshot-data.sh <tag>
#
# Environment: SNAPSHOT_PORT (the first of two ports, default 4791).
set -euo pipefail

tag="${1:?usage: $0 <tag, such as v0.3.0>}"
repository="$(cd "$(dirname "$0")/../.." && pwd)"
port_alice="${SNAPSHOT_PORT:-4791}"
port_bob="$((port_alice + 1))"
destination="$repository/testdata/releases/$tag/data"
client_destination="$repository/testdata/releases/$tag/client"

git -C "$repository" rev-parse --verify --quiet "refs/tags/$tag^{commit}" >/dev/null \
  || { echo "$tag is not a tag of this repository (git fetch --tags)" >&2; exit 1; }
for kept in "$destination" "$client_destination"; do
  [ ! -e "$kept" ] || echo "${kept#"$repository/"} is kept already; left as it is (remove it first to make it again)"
done
[ ! -e "$destination" ] || [ ! -e "$client_destination" ] || exit 0

work="$(mktemp -d)"
serve_pid=""
stop_serving() {
  [ -z "$serve_pid" ] || { kill "$serve_pid" 2>/dev/null || true; wait "$serve_pid" 2>/dev/null || true; }
  serve_pid=""
}
cleanup() {
  stop_serving
  git -C "$repository" worktree remove --force "$work/source" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

git -C "$repository" worktree add --quiet --detach "$work/source" "refs/tags/$tag"

if [ ! -e "$client_destination" ]; then
  [ -d "$work/source/testdata/client" ] \
    || { echo "$tag has no testdata/client" >&2; exit 1; }
  mkdir -p "$(dirname "$client_destination")"
  cp -R "$work/source/testdata/client" "$client_destination"
  echo "client answers of $tag: $client_destination"
fi
[ ! -e "$destination" ] || exit 0

target="${CARGO_TARGET_DIR:-$work/target}"
(cd "$work/source" && CARGO_TARGET_DIR="$target" cargo build --quiet --locked -p agency-node)
binary="$target/debug/agency"

# The fixed place the checked-in files say the runtime is at; nothing needs to exist there later.
fixture="$work/fixture"
mkdir -p "$fixture/state" "$work/home" "$work/xdg"
cp "$repository/scripts/release/fixture-runtime.sh" "$fixture/runtime.sh"

# What the node runs is the release's own binary, on a home and a configuration directory of its own, so
# that the org's root key (kept outside a data directory) is made in the throwaway directory.
agency() {
  HOME="$work/home" XDG_CONFIG_HOME="$work/xdg" "$binary" "$@"
}
alice() { agency --dir "$work/alice" "$@"; }
bob() { agency --dir "$work/bob" "$@"; }

alice init --account alice --label Alice --url "http://127.0.0.1:$port_alice" \
  --listen "127.0.0.1:$port_alice" --scrum "$fixture/runtime.sh" >/dev/null
bob init --account bob --label Bob --url "http://127.0.0.1:$port_bob" \
  --listen "127.0.0.1:$port_bob" --scrum "$fixture/runtime.sh" >/dev/null
alice org-create --org acme --label Acme >/dev/null
alice team-add --org acme --team app --label App --state-dir "$fixture/state" >/dev/null
alice device-add --cli >/dev/null
invite="$(alice invite --org acme)"
link="$(printf '%s\n' "$invite" | grep -o 'agency:join[^ ]*' | head -n 1)"
[ -n "$link" ] || { echo "no invite link in: $invite" >&2; exit 1; }

# The node itself is started here, not through the function above: `$!` must be its process, which `kill`
# stops, and not a subshell that would leave it running.
HOME="$work/home" XDG_CONFIG_HOME="$work/xdg" "$binary" --dir "$work/alice" serve >"$work/serve.log" 2>&1 &
serve_pid=$!
for _ in $(seq 1 50); do
  curl -sf "http://127.0.0.1:$port_alice/api/me" >/dev/null 2>&1 && break
  kill -0 "$serve_pid" 2>/dev/null || { echo "alice's node stopped:" >&2; cat "$work/serve.log" >&2; exit 1; }
  sleep 0.2
done

bob join "$link" --yes >/dev/null
bob device-add --cli >/dev/null
bob sync >/dev/null
stop_serving

alice decide --org acme --team app --request r-1 --choice yes >/dev/null

# Written where the repository does not see it, and moved in once all of it is made: a failure part way
# leaves nothing behind that the next run would refuse.
staged="$work/staged"
mkdir -p "$staged"
for node in alice bob; do
  cp -R "$work/$node" "$staged/$node"
  rm -rf "$staged/$node/logs"
  # git keeps no empty directory, so a snapshot has none either.
  find "$staged/$node" -type d -empty -delete
  # `-i.bak` is the form both GNU and BSD sed (macOS) take.
  sed -i.bak "s|$fixture|/fixture|g" "$staged/$node/config.json"
  rm "$staged/$node/config.json.bak"
done

# Nothing of this machine may be left in what is checked in.
if grep -rIl -e "$work" -e "$HOME" "$staged" >/dev/null; then
  echo "the snapshot still holds a path of this machine:" >&2
  grep -rIn -e "$work" -e "$HOME" "$staged" >&2
  exit 1
fi
mkdir -p "$(dirname "$destination")"
mv "$staged" "$destination"
echo "snapshot of $tag: $destination"
