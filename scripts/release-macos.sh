#!/bin/bash
# Builds the node on this Mac for a release and attaches it to the release with gh. The Linux builds are
# attached by CI; macOS builds are never made on GitHub's runners.
#
#   scripts/release-macos.sh <tag> [--skip-smoke]
#
# <tag> is the tag of a release that exists (release-please made it, for example v0.2.0). The working tree
# must be clean and at that tag (`git fetch --tags && git checkout <tag>`), so that the binary is the
# released source. The script builds the web client and the node, checks that the binary starts and serves
# on a throwaway data directory (--skip-smoke leaves that out), packs it as
# agency-<version>-macos-<arm64|x86_64>.tar.gz with its .sha256, and uploads both to the release (a file
# of the same name is replaced). The binary is not signed and not notarized.
set -euo pipefail
cd "$(dirname "$0")/.."

usage() {
  echo "usage: $0 <tag> [--skip-smoke]" >&2
  exit 2
}
tag="" smoke=1
while [ $# -gt 0 ]; do
  case "$1" in
    --skip-smoke) smoke=0; shift ;;
    -*) usage ;;
    *) [ -z "$tag" ] || usage; tag="$1"; shift ;;
  esac
done
[ -n "$tag" ] || usage
fail() { echo "release-macos: $*" >&2; exit 1; }

[ "$(uname -s)" = Darwin ] || fail "this builds the macOS binary, and so must run on a Mac"
case "$(uname -m)" in arm64) arch=arm64 ;; x86_64) arch=x86_64 ;; *) fail "unknown architecture $(uname -m)" ;; esac
case "$tag" in v[0-9]*.[0-9]*.[0-9]*) ;; *) fail "$tag is not a version tag such as v0.2.0" ;; esac

tag_commit="$(git rev-parse -q --verify "refs/tags/$tag^{commit}")" || fail "there is no tag $tag here; run: git fetch --tags"
[ "$(git rev-parse HEAD)" = "$tag_commit" ] || fail "this checkout is not at $tag; run: git checkout $tag"
[ -z "$(git status --porcelain)" ] || fail "the working tree has changes; the binary must be built from the tag as it is"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
[ "v$version" = "$tag" ] || fail "Cargo.toml says version $version, not ${tag#v}"
gh release view "$tag" >/dev/null || fail "there is no GitHub Release for $tag yet; merge the release pull request first"

echo "release-macos: building $tag for macOS $arch"
(cd web && npm ci && npm run build)
cargo build --release --locked -p agency-node
binary="$(scripts/release/target-dir.sh .)/release/agency"
[ -x "$binary" ] || fail "the build made no $binary"
if [ "$smoke" -eq 1 ]; then scripts/release/smoke-test.sh "$binary"; fi

out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
scripts/release/package.sh "$binary" macos "$arch" "$out" --release
gh release upload "$tag" "$out"/*.tar.gz "$out"/*.sha256 --clobber
echo "release-macos: attached the macOS $arch build to $tag"
