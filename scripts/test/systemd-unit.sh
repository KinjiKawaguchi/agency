#!/bin/bash
# Tests scripts/systemd-unit.sh: the unit it writes keeps paths with spaces and with characters that
# systemd reads specially (`%`, `$`, `"`, `\`) as they are. On a machine with systemd, it also asks
# `systemd-analyze` to verify the unit.
set -euo pipefail
cd "$(dirname "$0")/../.."

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0
check() { # check <description> <expected line> <file>
  if ! grep -qxF -- "$2" "$3"; then
    echo "FAIL: $1" >&2; echo "  expected: $2" >&2; echo "  in:" >&2; sed 's/^/    /' "$3" >&2
    failures=$((failures + 1))
  fi
}

# A data directory and a binary whose names need quoting.
data="$work/data dir 100%"
mkdir -p "$data" "$work/bin dir"
echo '{}' > "$data/config.json"
printf '#!/bin/sh\nexit 0\n' > "$work/bin dir/agency"
chmod +x "$work/bin dir/agency"
real_data="$(cd "$data" && pwd -P)"
real_bin="$(cd "$work/bin dir" && pwd -P)/agency"

# shellcheck disable=SC2016  # the $ is meant literally: systemd must not expand it
odd_path='/usr/bin:/bin:/opt/my tools/bin:/opt/100%/bin:/opt/$HOME/bin'
PATH="$odd_path" scripts/systemd-unit.sh --agency "$work/bin dir/agency" --dir "$data" --out "$work/out/agency-node.service" >/dev/null
unit="$work/out/agency-node.service"

check "the command line quotes both paths and doubles %" \
  "ExecStart=\"${real_bin//%/%%}\" --dir \"${real_data//%/%%}\" serve" "$unit"
check "the working directory is the data directory, with % doubled" "WorkingDirectory=${real_data//%/%%}" "$unit"
# shellcheck disable=SC2016
check "the PATH is the PATH of the shell, with % doubled and spaces kept" \
  'Environment="PATH=/usr/bin:/bin:/opt/my tools/bin:/opt/100%%/bin:/opt/$HOME/bin"' "$unit"
check "the log goes to the data directory" "StandardOutput=append:${real_data//%/%%}/logs/serve.log" "$unit"
check "it restarts" "Restart=always" "$unit"
check "it starts with the user manager" "WantedBy=default.target" "$unit"
[ -d "$real_data/logs" ] || { echo "FAIL: the logs directory was not made" >&2; failures=$((failures + 1)); }

# A name with a double quote and a backslash, and `$` on the command line (doubled).
quoted="$work/q\"u\\o\$te"
mkdir -p "$quoted"
echo '{}' > "$quoted/config.json"
scripts/systemd-unit.sh --agency "$work/bin dir/agency" --dir "$quoted" --out "$work/quoted.service" >/dev/null
real_quoted="$(cd "$quoted" && pwd -P)"
escaped="${real_quoted//\\/\\\\}"; escaped="${escaped//\"/\\\"}"; escaped="${escaped//\$/\$\$}"
check "a quote, a backslash and a dollar sign are escaped" "ExecStart=\"${real_bin}\" --dir \"${escaped}\" serve" "$work/quoted.service"

# It refuses what it cannot write a unit for.
if scripts/systemd-unit.sh --agency "$work/missing" --dir "$data" --out "$work/x.service" 2>/dev/null; then
  echo "FAIL: a binary that is not there was accepted" >&2; failures=$((failures + 1))
fi
mkdir "$work/empty"
if scripts/systemd-unit.sh --agency "$work/bin dir/agency" --dir "$work/empty" --out "$work/x.service" 2>/dev/null; then
  echo "FAIL: a data directory without config.json was accepted" >&2; failures=$((failures + 1))
fi
if PATH=$'/usr/bin\n/x' scripts/systemd-unit.sh --agency "$work/bin dir/agency" --dir "$data" --out "$work/x.service" 2>/dev/null; then
  echo "FAIL: a PATH with a line break was accepted" >&2; failures=$((failures + 1))
fi

if command -v systemd-analyze >/dev/null 2>&1; then
  plain="$work/plain"
  mkdir -p "$plain/data"
  echo '{}' > "$plain/data/config.json"
  cp "$work/bin dir/agency" "$plain/agency"
  scripts/systemd-unit.sh --agency "$plain/agency" --dir "$plain/data" --out "$plain/agency-node.service" >/dev/null
  systemd-analyze --user verify "$plain/agency-node.service" || { echo "FAIL: systemd-analyze refused the unit" >&2; failures=$((failures + 1)); }
fi

[ "$failures" -eq 0 ] && echo "systemd-unit: ok"
exit "$failures"
