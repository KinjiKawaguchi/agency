#!/bin/bash
# Tests the Debian package script with a dynamically linked system binary, checking its metadata,
# contents, checksum, and rejection of an unsupported architecture.
set -euo pipefail
cd "$(dirname "$0")/../.."

command -v dpkg-deb >/dev/null || { echo "debian-package: skipped (dpkg-deb is unavailable)"; exit 0; }
command -v readelf >/dev/null || { echo "debian-package: skipped (readelf is unavailable)"; exit 0; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }
expect() { [ "$2" = "$3" ] || fail "$1: got '$2', wanted '$3'"; }
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"

scripts/release/debian-package.sh /bin/true x86_64 "$work/out" >/dev/null
package="$(find "$work/out" -name '*.deb' -print -quit)"
[ -n "$package" ] || { fail "the package was not written"; exit "$failures"; }
expect "the package is for amd64" "$(dpkg-deb -f "$package" Architecture)" amd64
expect "the maintainer is the project" "$(dpkg-deb -f "$package" Maintainer)" "Agency Project <agency@agency.invalid>"
expect "the package describes the node" "$(dpkg-deb -f "$package" Description | head -n 1)" "node of a self-hosted organization of AI agents and people"
expect "a non-release build records its commit" "$(dpkg-deb -f "$package" Version | sed "s/$version//")" "+git$(git rev-parse --short=7 HEAD)"
case "$(dpkg-deb -f "$package" Depends)" in *libc6\ \(\>=\ [0-9]*\.[0-9]*\)*) ;; *) fail "the package does not depend on the binary's libc: $(dpkg-deb -f "$package" Depends)" ;; esac
case "$(dpkg-deb -f "$package" Depends)" in *ca-certificates*) ;; *) fail "the package does not depend on the trusted root certificates: $(dpkg-deb -f "$package" Depends)" ;; esac
contents="$(dpkg-deb --contents "$package")"
for path in usr/bin/agency usr/share/doc/agency/install.md usr/share/doc/agency/install.ja.md usr/share/doc/agency/LICENSE-MIT usr/share/doc/agency/LICENSE-APACHE usr/share/doc/agency/copyright usr/share/doc/agency/changelog.gz usr/share/doc/agency/systemd-unit.sh; do
  case "$contents" in *"./$path"*) ;; *) fail "the package does not contain $path" ;; esac
done
case "$contents" in *"./lib/systemd/"* | *"./usr/lib/systemd/"*) fail "the package installs a system unit" ;; esac
(cd "$work/out" && sha256sum -c "$(basename "$package").sha256" >/dev/null) || fail "the checksum does not match the package"
if scripts/release/debian-package.sh /bin/true unsupported "$work/out" >/dev/null 2>&1; then
  fail "an unsupported architecture was accepted"
fi
if scripts/release/debian-package.sh /bin/true arm64 "$work/out" >/dev/null 2>&1; then
  fail "a binary for another architecture was accepted"
fi

# A release package gets its version from the tag at HEAD, rather than a generated build version.
release_repo="$work/release"
mkdir -p "$release_repo/scripts/release"
cp Cargo.toml LICENSE-MIT LICENSE-APACHE "$release_repo/"
cp -R docs "$release_repo/docs"
cp scripts/systemd-unit.sh "$release_repo/scripts/"
cp scripts/release/debian-package.sh "$release_repo/scripts/release/"
(
  cd "$release_repo"
  git init -q -b main
  git add -A
  git -c user.name=test -c user.email=test@example.invalid -c commit.gpgsign=false commit -q -m "test: package"
  git -c tag.gpgsign=false tag "v$version"
  scripts/release/debian-package.sh /bin/true x86_64 "$work/release-out" --release >/dev/null
)
release_package="$(find "$work/release-out" -name '*.deb' -print -quit)"
expect "a release package uses its tag as its version" "$(dpkg-deb -f "$release_package" Version)" "$version"

[ "$failures" -eq 0 ] && echo "debian-package: ok"
exit "$failures"
