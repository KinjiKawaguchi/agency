#!/bin/bash
# Tests scripts/release-macos.sh and scripts/release/package.sh against stand-ins for gh, cargo, npm, and
# uname (scripts/test/release-fakes), in a throwaway repository with a tag.
set -euo pipefail
cd "$(dirname "$0")/../.."
fakes="$PWD/scripts/test/release-fakes"
source_dir="$PWD"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }
expect() { [ "$2" = "$3" ] || fail "$1: got '$2', wanted '$3'"; }

# A repository that holds what the scripts use, committed and tagged vVERSION.
fresh() {
  rm -rf "$work/repo" "$work/state"; mkdir -p "$work/repo/scripts" "$work/state"
  FAKE_STATE="$work/state"; export FAKE_STATE
  unset FAKE_NO_RELEASE FAKE_ARCH
  cp "$source_dir/scripts/release-macos.sh" "$source_dir/scripts/launchd-plist.sh" "$source_dir/scripts/deploy-macos.sh" "$source_dir/scripts/systemd-unit.sh" "$work/repo/scripts/"
  cp -R "$source_dir/scripts/release" "$work/repo/scripts/release"
  cp -R "$source_dir/docs" "$work/repo/docs"
  cp "$source_dir"/LICENSE-* "$source_dir/Cargo.toml" "$work/repo/"
  mkdir -p "$work/repo/web"
  (cd "$work/repo" && git init -q -b main && git add -A && git -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false commit -q -m "feat: x" \
    && git -c user.name=t -c user.email=t@example.com -c tag.gpgsign=false tag "v$version")
}
release() { # release <args>; sets status and output
  set +e
  output="$(cd "$work/repo" && PATH="$fakes:$PATH" scripts/release-macos.sh "$@" 2>&1)"
  status=$?
  set -e
}

fresh
release "v$version" --skip-smoke
expect "a release build exits 0" "$status" 0
name="agency-$version-macos-arm64"
expect "the tarball and its checksum are uploaded, replacing earlier ones" "$(sed 's#[^ ]*/##g' "$FAKE_STATE/uploads")" "v$version $name.tar.gz $name.tar.gz.sha256 --clobber"
expect "the web client is built before the node" "$(cat "$FAKE_STATE/npm")" "$(printf 'ci\nrun build')"
expect "the node is built from the lockfile" "$(cat "$FAKE_STATE/cargo")" "build --release --locked -p agency-node"

# What the tarball holds, from package.sh run the same way.
out="$work/out"; mkdir -p "$out"
(cd "$work/repo" && PATH="$fakes:$PATH" scripts/release/package.sh target/release/agency macos x86_64 "$out" --release >/dev/null)
expect "a macOS tarball holds the binary, the launchd and deploy scripts, the guides, and the licenses" \
  "$(tar tzf "$out/agency-$version-macos-x86_64.tar.gz" | sed 's#^[^/]*/##' | LC_ALL=C sort | tr '\n' ' ')" \
  " LICENSE-APACHE LICENSE-MIT agency deploy-macos.sh install.ja.md install.md launchd-plist.sh "
(cd "$work/repo" && scripts/release/package.sh target/release/agency linux arm64 "$out" >/dev/null)
linux=""
for candidate in "$out"/agency-"$version"-???????-linux-arm64.tar.gz; do
  [ ! -e "$candidate" ] || linux="$(basename "$candidate")"
done
[ -n "$linux" ] || fail "a build that is not of a release carries the short commit in its name"
expect "a Linux tarball holds the systemd unit script" \
  "$(tar tzf "$out/$linux" | grep -c systemd-unit.sh)" 1
(cd "$out" && { sha256sum -c "agency-$version-macos-x86_64.tar.gz.sha256" 2>/dev/null || shasum -a 256 -c "agency-$version-macos-x86_64.tar.gz.sha256"; } >/dev/null) || fail "the checksum does not match the tarball"

fresh; FAKE_ARCH=x86_64 release "v$version" --skip-smoke
expect "an Intel Mac names its build x86_64" "$(sed 's#[^ ]*/##g' "$FAKE_STATE/uploads" | cut -d' ' -f2)" "agency-$version-macos-x86_64.tar.gz"

fresh; FAKE_NO_RELEASE=1 release "v$version" --skip-smoke
expect "no GitHub Release yet" "$status" 1
[ ! -e "$FAKE_STATE/uploads" ] && [ ! -e "$FAKE_STATE/cargo" ] || fail "something was built or uploaded without a release"

fresh; echo "x" > "$work/repo/dirty"
release "v$version" --skip-smoke
expect "a dirty working tree is refused" "$status" 1

fresh; (cd "$work/repo" && git -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false commit -q --allow-empty -m "feat: y")
release "v$version" --skip-smoke
expect "a checkout that is not at the tag is refused" "$status" 1
case "$output" in *"git checkout v$version"*) ;; *) fail "the output does not say how to check out the tag: $output" ;; esac

fresh; release "v9.9.9" --skip-smoke
expect "a tag that does not exist is refused" "$status" 1

fresh; (cd "$work/repo" && git tag v0.0.1 && git checkout -q v0.0.1)
release "v0.0.1" --skip-smoke
expect "a tag that does not match Cargo.toml is refused" "$status" 1

fresh; release "$version" --skip-smoke
expect "a name that is not a version tag is refused" "$status" 1

fresh; release
expect "no tag is a usage error" "$status" 2

[ "$failures" -eq 0 ] && echo "release-macos: ok"
exit "$failures"
