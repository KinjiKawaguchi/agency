#!/bin/bash
# Writes SHA256SUMS: the SHA-256 of every file of a release directory, in the format `sha256sum --check`
# reads. The release workflow signs this file, so that one signature covers every build and every list of
# materials in the release.
#
#   scripts/release/checksums.sh <directory>
#
# The per-file `.sha256` files and an earlier SHA256SUMS (and its signature) are left out.
set -euo pipefail

[ $# -eq 1 ] && [ -d "$1" ] || { echo "usage: $0 <directory>" >&2; exit 2; }
cd "$1"
files="$(find . -maxdepth 1 -type f ! -name 'SHA256SUMS*' ! -name '*.sha256' -printf '%f\n' | LC_ALL=C sort)"
[ -n "$files" ] || { echo "no files to sum in $1" >&2; exit 1; }
printf '%s\n' "$files" | xargs -d '\n' sha256sum > SHA256SUMS
echo "Wrote $1/SHA256SUMS ($(wc -l < SHA256SUMS | tr -d ' ') files)"
