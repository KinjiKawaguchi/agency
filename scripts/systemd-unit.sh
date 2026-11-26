#!/bin/bash
# Writes a systemd user unit that runs `agency serve` and starts it again if it stops (Linux).
#
#   scripts/systemd-unit.sh --agency <agency binary> --dir <data directory> --out <unit file>
#
# The unit runs with the PATH of the shell that runs this script, so the team runtime finds the same
# tools as your terminal does (a systemd user service otherwise gets a short default PATH). Its output
# goes to <data directory>/logs/serve.log. This script only writes the file; docs/install.md says how to
# load it (`systemctl --user enable --now <unit name>`) and how to keep it running after you log out.
set -euo pipefail

usage() {
  echo "usage: $0 --agency <agency binary> --dir <data directory> --out <unit file>" >&2
  exit 2
}

agency="" dir="" out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --agency) agency="${2:-}"; shift 2 ;;
    --dir) dir="${2:-}"; shift 2 ;;
    --out) out="${2:-}"; shift 2 ;;
    *) usage ;;
  esac
done
[ -n "$agency" ] && [ -n "$dir" ] && [ -n "$out" ] || usage

# The unit starts the node in the data directory, so every path in the file is absolute.
absolute_dir() { (cd "$1" && pwd -P); }
[ -x "$agency" ] || { echo "$agency is not an executable file" >&2; exit 1; }
[ -f "$dir/config.json" ] || { echo "$dir has no config.json; run \`agency --dir $dir init\` first" >&2; exit 1; }
agency="$(absolute_dir "$(dirname "$agency")")/$(basename "$agency")"
dir="$(absolute_dir "$dir")"
mkdir -p "$dir/logs" "$(dirname "$out")"
log="$dir/logs/serve.log"

# A newline would end the line it is in.
for value in "$agency" "$dir" "$PATH"; do
  case "$value" in *$'\n'*) echo "a path with a line break cannot go in a unit" >&2; exit 1 ;; esac
done

# systemd reads `%` as the start of a specifier everywhere in a unit and `$` as a variable in a command
# line; a double-quoted word keeps its spaces, with `\` and `"` escaped.
specifiers() { printf '%s' "${1//%/%%}"; }
word() {
  local text
  text="${1//\\/\\\\}"
  text="${text//\"/\\\"}"
  text="${text//\$/\$\$}"
  printf '"%s"' "$(specifiers "$text")"
}
assignment() {
  local text
  text="${1//\\/\\\\}"
  text="${text//\"/\\\"}"
  printf '"%s"' "$(specifiers "$text")"
}

temporary="$out.tmp.$$"
trap 'rm -f "$temporary"' EXIT
{
  echo "[Unit]"
  echo "Description=Agency node"
  echo
  echo "[Service]"
  echo "ExecStart=$(word "$agency") --dir $(word "$dir") serve"
  echo "WorkingDirectory=$(specifiers "$dir")"
  echo "Environment=$(assignment "PATH=$PATH")"
  echo "Restart=always"
  echo "RestartSec=10"
  echo "StandardOutput=append:$(specifiers "$log")"
  echo "StandardError=append:$(specifiers "$log")"
  echo
  echo "[Install]"
  echo "WantedBy=default.target"
} > "$temporary"
mv "$temporary" "$out"
trap - EXIT

echo "Wrote $out (log $log)"
