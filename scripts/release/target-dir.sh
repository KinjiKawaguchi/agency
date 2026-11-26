#!/bin/bash
# Prints the directory cargo builds into, as cargo says it (`target_directory` of `cargo metadata`):
# `target` in the repository, unless CARGO_TARGET_DIR or a cargo configuration puts it elsewhere.
#
#   scripts/release/target-dir.sh [repository]
#
# The repository is the one this script is in unless one is given.
set -euo pipefail

repository="${1:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$repository"
directory="$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
[ -n "$directory" ] || { echo "cannot read the target directory from cargo metadata" >&2; exit 1; }
printf '%s\n' "$directory"
