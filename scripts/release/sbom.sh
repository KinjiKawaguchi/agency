#!/bin/bash
# Writes the software bills of materials (CycloneDX, JSON) of a release: what the node is made of.
#
#   scripts/release/sbom.sh <output directory> [--release]
#
# Writes <output directory>/agency-<version>[-<short commit>]-sbom-rust.cdx.json (the crates of the node,
# from Cargo.lock, for every platform the node is built for, so it may name a crate that one build does not
# contain) and ...-sbom-web.cdx.json (the npm packages the web client embeds, from web/package-lock.json,
# without the development tools that only test and build). The short commit is left out with --release,
# which names a build of a tagged release, as package.sh does.
#
# It needs cargo-cyclonedx (`cargo cyclonedx`), npm (9 or later), and jq. It fails when a list holds no
# components or is not CycloneDX, so that an empty list is never attached to a release.
set -euo pipefail
cd "$(dirname "$0")/../.."

usage() {
  echo "usage: $0 <output directory> [--release]" >&2
  exit 2
}
[ $# -ge 1 ] && [ $# -le 2 ] || usage
out="$1" release=0
if [ $# -eq 2 ]; then [ "$2" = "--release" ] || usage; release=1; fi

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
name="agency-$version"
if [ "$release" -eq 0 ]; then name="$name-$(git rev-parse --short=7 HEAD)"; fi

# Fails unless the file is a CycloneDX list with at least one component.
check_list() {
  jq -e '.bomFormat == "CycloneDX" and ((.components // []) | length) > 0' "$1" >/dev/null \
    || { echo "$1 is not a CycloneDX list with components" >&2; exit 1; }
}

mkdir -p "$out"
rust_list="$out/$name-sbom-rust.cdx.json"
web_list="$out/$name-sbom-web.cdx.json"

# cargo-cyclonedx writes one file next to each crate of the workspace; the node's holds all it depends on.
cargo cyclonedx --manifest-path crates/agency-node/Cargo.toml --format json --spec-version 1.5 --target all
cp crates/agency-node/agency-node.cdx.json "$rust_list"
rm -f crates/*/*.cdx.json
check_list "$rust_list"

(cd web && npm sbom --package-lock-only --omit dev --sbom-format cyclonedx) > "$web_list"
check_list "$web_list"

echo "Wrote $rust_list and $web_list"
