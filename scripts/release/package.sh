#!/bin/bash
# Packs a release build: the binary, the script that runs it as a service, the install guides, and the
# licenses.
#
#   scripts/release/package.sh <agency binary> <linux|macos> <x86_64|arm64> <output directory> [--release]
#
# Writes <output directory>/agency-<version>[-<short commit>]-<os>-<arch>.tar.gz and its .sha256. The short
# commit is left out with --release, which names a build of a tagged release. When run by GitHub Actions,
# the artifact's name goes to `name=` in $GITHUB_OUTPUT.
set -euo pipefail
cd "$(dirname "$0")/../.."

usage() {
  echo "usage: $0 <agency binary> <linux|macos> <x86_64|arm64> <output directory> [--release]" >&2
  exit 2
}
[ $# -ge 4 ] && [ $# -le 5 ] || usage
binary="$1" os="$2" arch="$3" out="$4" release=0
if [ $# -eq 5 ]; then [ "$5" = "--release" ] || usage; release=1; fi
case "$os" in linux | macos) ;; *) usage ;; esac
case "$arch" in x86_64 | arm64) ;; *) usage ;; esac
[ -x "$binary" ] || { echo "$binary is not an executable file" >&2; exit 1; }

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
name="agency-$version"
if [ "$release" -eq 0 ]; then name="$name-$(git rev-parse --short=7 HEAD)"; fi
name="$name-$os-$arch"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/$name" "$out"
install -m 755 "$binary" "$stage/$name/agency"
case "$os" in
  linux) service_scripts=(scripts/systemd-unit.sh) ;;
  macos) service_scripts=(scripts/launchd-plist.sh scripts/deploy-macos.sh) ;;
esac
for file in "${service_scripts[@]}"; do
  install -m 755 "$file" "$stage/$name/$(basename "$file")"
done
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
