#!/bin/bash
# Writes a launchd agent that runs `agency serve` at login and restarts it if it stops (macOS).
#
#   scripts/launchd-plist.sh --agency <agency binary> --dir <data directory> --out <plist> [--label <label>]
#
# The agent runs with the PATH of the shell that runs this script, so the team runtime finds the same
# tools as your terminal does. Its output goes to <data directory>/logs/serve.log.
# This script only writes the file; load it with `launchctl bootstrap gui/$(id -u) <plist>`.
set -euo pipefail

usage() {
  echo "usage: $0 --agency <agency binary> --dir <data directory> --out <plist> [--label <label>]" >&2
  exit 2
}

agency="" dir="" out="" label="local.agency.node"
while [ $# -gt 0 ]; do
  case "$1" in
    --agency) agency="${2:-}"; shift 2 ;;
    --dir) dir="${2:-}"; shift 2 ;;
    --out) out="${2:-}"; shift 2 ;;
    --label) label="${2:-}"; shift 2 ;;
    *) usage ;;
  esac
done
[ -n "$agency" ] && [ -n "$dir" ] && [ -n "$out" ] || usage

# launchd starts the agent in `/`, so every path in the file is absolute.
absolute_dir() { (cd "$1" && pwd -P); }
[ -x "$agency" ] || { echo "$agency is not an executable file" >&2; exit 1; }
[ -f "$dir/config.json" ] || { echo "$dir has no config.json; run \`agency --dir $dir init\` first" >&2; exit 1; }
agency="$(absolute_dir "$(dirname "$agency")")/$(basename "$agency")"
dir="$(absolute_dir "$dir")"
mkdir -p "$dir/logs" "$(dirname "$out")"
log="$dir/logs/serve.log"

# plutil writes the XML, so no value needs escaping here.
plutil -create xml1 "$out"
plutil -insert Label -string "$label" "$out"
plutil -insert ProgramArguments -array "$out"
for argument in "$agency" --dir "$dir" serve; do
  plutil -insert ProgramArguments -string "$argument" -append "$out"
done
plutil -insert EnvironmentVariables -dictionary "$out"
plutil -insert EnvironmentVariables.PATH -string "$PATH" "$out"
plutil -insert WorkingDirectory -string "$dir" "$out"
plutil -insert RunAtLoad -bool YES "$out"
plutil -insert KeepAlive -bool YES "$out"
plutil -insert ThrottleInterval -integer 10 "$out"
plutil -insert StandardOutPath -string "$log" "$out"
plutil -insert StandardErrorPath -string "$log" "$out"
plutil -lint "$out" >/dev/null

echo "Wrote $out (label $label, log $log)"
