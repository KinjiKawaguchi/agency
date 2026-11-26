#!/bin/bash
# Packs a release build for Linux: the binary, the systemd unit script, the install guides, and the licenses.
#
#   scripts/release/package-linux.sh <agency binary> <x86_64|arm64> <output directory>
#
# Writes <output directory>/agency-<version>-<short commit>-linux-<arch>.tar.gz and its .sha256, and, when
# run by GitHub Actions, the artifact's name as `name=` in $GITHUB_OUTPUT.
set -euo pipefail
cd "$(dirname "$0")/../.."

[ $# -eq 3 ] || { echo "usage: $0 <agency binary> <x86_64|arm64> <output directory>" >&2; exit 2; }
binary="$1" arch="$2" out="$3"
case "$arch" in x86_64 | arm64) ;; *) echo "arch must be x86_64 or arm64" >&2; exit 2 ;; esac
[ -x "$binary" ] || { echo "$binary is not an executable file" >&2; exit 1; }

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
commit="$(git rev-parse --short=7 HEAD)"
name="agency-$version-$commit-linux-$arch"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

mkdir -p "$stage/$name" "$out"
install -m 755 "$binary" "$stage/$name/agency"
install -m 755 scripts/systemd-unit.sh "$stage/$name/systemd-unit.sh"
for file in docs/install.md docs/install.ja.md LICENSE-MIT LICENSE-APACHE; do
  install -m 644 "$file" "$stage/$name/$(basename "$file")"
done
tar -C "$stage" -czf "$out/$name.tar.gz" "$name"
if command -v sha256sum >/dev/null; then
  (cd "$out" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
else
  (cd "$out" && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
fi
echo "Wrote $out/$name.tar.gz"
[ -z "${GITHUB_OUTPUT:-}" ] || echo "name=$name" >> "$GITHUB_OUTPUT"
