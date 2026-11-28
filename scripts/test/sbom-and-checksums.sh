#!/bin/bash
# Tests scripts/release/sbom.sh against stand-ins for `cargo cyclonedx` and `npm sbom`, and
# scripts/release/checksums.sh on a directory of files, both in a throwaway copy of the repository.
#
#   scripts/test/sbom-and-checksums.sh
#
# SBOM_SCRIPT and CHECKSUMS_SCRIPT name the scripts to test instead of the real ones, so that a mutated copy
# can be tried.
set -euo pipefail
cd "$(dirname "$0")/../.."
fakes="$PWD/scripts/test/sbom-fakes"
sbom_script="${SBOM_SCRIPT:-$PWD/scripts/release/sbom.sh}"
checksums_script="${CHECKSUMS_SCRIPT:-$PWD/scripts/release/checksums.sh}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }
expect() { [ "$2" = "$3" ] || fail "$1: got '$2', wanted '$3'"; }

# A repository with the files the script reads, the scripts directory, and one commit.
repo="$work/repo"
mkdir -p "$repo/scripts/release" "$repo/web" "$repo/state"
for crate in agency-proto agency-core agency-adapters agency-node; do mkdir -p "$repo/crates/$crate"; done
printf '[workspace.package]\nversion = "1.2.3"\n' > "$repo/Cargo.toml"
cp "$sbom_script" "$repo/scripts/release/sbom.sh"
chmod +x "$repo/scripts/release/sbom.sh"
git -C "$repo" init -q -b main
git -C "$repo" -c user.email=t@example.test -c user.name=t -c commit.gpgsign=false commit -q --allow-empty -m "chore: base"
commit="$(git -C "$repo" rev-parse --short=7 HEAD)"

# names: the file names in the output directory, one per line, in order.
names() { find "$repo/out" -type f -printf '%f\n' | LC_ALL=C sort; }

# sbom <mode> [arguments]: runs the script, and sets status and output.
sbom() {
  rm -rf "$repo/out" "$repo/state"; mkdir -p "$repo/state"
  local mode="$1"; shift
  status=0
  output="$(cd "$repo" && PATH="$fakes:$PATH" FAKE_STATE="$repo/state" FAKE_MODE="$mode" scripts/release/sbom.sh "$@" 2>&1)" || status=$?
}

sbom ok out
expect "a build that is not of a release: exit status" "$status" 0
expect "the Rust list is named with the version and the commit" "$(names | head -n 1)" "agency-1.2.3-$commit-sbom-rust.cdx.json"
expect "the web list too" "$(names | tail -n 1)" "agency-1.2.3-$commit-sbom-web.cdx.json"
expect "the Rust list is the node's, not another crate's" "$(jq -r '.components[0].name' "$repo/out/"*-rust.cdx.json)" "dep-of-agency-node"
expect "the web list is npm's" "$(jq -r '.components[0].name' "$repo/out/"*-web.cdx.json)" "a-web-package"
expect "the lists of the other crates are not left in the crates" "$(find "$repo/crates" -name '*.cdx.json' | wc -l | tr -d ' ')" 0
case "$(cat "$repo/state/cargo")" in *"--manifest-path crates/agency-node/Cargo.toml"*"--target all"*) ;; *) fail "cargo cyclonedx was not asked for the node, for every platform: $(cat "$repo/state/cargo")" ;; esac
case "$(cat "$repo/state/npm")" in "sbom --package-lock-only --omit dev --sbom-format cyclonedx") ;; *) fail "npm sbom was not asked for the production packages from the lock file: $(cat "$repo/state/npm")" ;; esac

sbom ok out --release
expect "a release build is named by the version alone" "$(names | head -n 1)" "agency-1.2.3-sbom-rust.cdx.json"

sbom empty out --release
expect "a Rust list with no components fails" "$status" 1
case "$output" in *"is not a CycloneDX list with components"*) ;; *) fail "the output does not say why: $output" ;; esac
sbom other out --release
expect "a Rust list that is not CycloneDX fails" "$status" 1
sbom web-empty out --release
expect "a web list with no components fails" "$status" 1
sbom ok
expect "no output directory is a usage error" "$status" 2
sbom ok out --other
expect "an unknown flag is a usage error" "$status" 2

# Checksums.
dir="$work/dist"
mkdir -p "$dir"
printf 'tarball' > "$dir/agency-1.2.3-linux-x86_64.tar.gz"
printf 'package' > "$dir/agency_1.2.3_amd64.deb"
printf '{}' > "$dir/agency-1.2.3-sbom-web.cdx.json"
printf 'x  y\n' > "$dir/agency-1.2.3-linux-x86_64.tar.gz.sha256"
printf 'old' > "$dir/SHA256SUMS"
printf 'old' > "$dir/SHA256SUMS.sigstore.json"
"$checksums_script" "$dir" >/dev/null
expect "the sums name every file and no checksum file, in order" "$(awk '{print $2}' "$dir/SHA256SUMS" | tr '\n' ' ')" "agency-1.2.3-linux-x86_64.tar.gz agency-1.2.3-sbom-web.cdx.json agency_1.2.3_amd64.deb "
(cd "$dir" && sha256sum --check --strict SHA256SUMS >/dev/null 2>&1) || fail "sha256sum does not accept the file"
printf 'changed' > "$dir/agency_1.2.3_amd64.deb"
(cd "$dir" && sha256sum --check SHA256SUMS >/dev/null 2>&1) && fail "a changed file passes the check"
expect "an earlier signature is not summed" "$(grep -c sigstore "$dir/SHA256SUMS" || true)" 0
mkdir -p "$work/empty"
status=0; output="$("$checksums_script" "$work/empty" 2>&1)" || status=$?
expect "a directory with no files fails" "$status" 1
case "$output" in *"no files to sum"*) ;; *) fail "the output does not say there was nothing to sum: $output" ;; esac
if "$checksums_script" "$work/missing" >/dev/null 2>&1; then fail "a directory that is not there was accepted"; fi

[ "$failures" -eq 0 ] && echo "sbom-and-checksums: ok"
exit "$failures"
