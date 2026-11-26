#!/bin/bash
# Packs a Linux release binary as a Debian package. The package installs only the node and its
# documentation: each person creates and runs their own systemd user unit.
#
#   scripts/release/debian-package.sh <agency binary> <x86_64|arm64> <output directory> [--release]
#
# Writes <output directory>/agency_<version>_<architecture>.deb and its .sha256. With --release,
# the version comes from the v<version> tag at HEAD. A non-release build has a Debian version that
# sorts after that release and carries the short commit.
set -euo pipefail
cd "$(dirname "$0")/../.."

usage() {
  echo "usage: $0 <agency binary> <x86_64|arm64> <output directory> [--release]" >&2
  exit 2
}
[ $# -ge 3 ] && [ $# -le 4 ] || usage
binary="$1" arch="$2" out="$3" release=0
if [ $# -eq 4 ]; then [ "$4" = "--release" ] || usage; release=1; fi
case "$arch" in
  x86_64) deb_arch=amd64 ;;
  arm64) deb_arch=arm64 ;;
  *) usage ;;
esac
[ -x "$binary" ] || { echo "$binary is not an executable file" >&2; exit 1; }
command -v dpkg-deb >/dev/null || { echo "dpkg-deb is required" >&2; exit 1; }
command -v readelf >/dev/null || { echo "readelf is required" >&2; exit 1; }
machine="$(readelf --file-header "$binary" | sed -n 's/^  Machine: *//p')"
case "$arch:$machine" in
  x86_64:Advanced\ Micro\ Devices\ X86-64 | arm64:AArch64) ;;
  *) echo "$binary is not a $arch Linux executable" >&2; exit 1 ;;
esac

cargo_version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
if [ "$release" -eq 1 ]; then
  tag="$(git describe --exact-match --tags)" || {
    echo "HEAD is not at a release tag" >&2
    exit 1
  }
  case "$tag" in
    "v$cargo_version") version="$cargo_version" ;;
    *) echo "release tag $tag does not match Cargo.toml version $cargo_version" >&2; exit 1 ;;
  esac
else
  version="$cargo_version+git$(git rev-parse --short=7 HEAD)"
fi

# The newest GLIBC symbol version is the oldest libc that can run this binary.
glibc_version="$(readelf --version-info --wide "$binary" \
  | sed -n 's/.*GLIBC_\([0-9][0-9.]*\).*/\1/p' | sort -Vu | tail -n 1)"
[ -n "$glibc_version" ] || { echo "$binary has no GLIBC version requirement" >&2; exit 1; }
needs=(libc6)
while read -r library; do
  case "$library" in
    libc.so.6 | libm.so.6 | libdl.so.2 | libpthread.so.0 | librt.so.1 | libutil.so.1 | ld-linux-*.so.*) ;;
    libgcc_s.so.1) needs+=(libgcc-s1) ;;
    *) echo "unmapped dynamic dependency: $library" >&2; exit 1 ;;
  esac
done < <(readelf --dynamic --wide "$binary" | sed -n 's/.*Shared library: \[\([^]]*\)\].*/\1/p')
dependencies=("libc6 (>= $glibc_version)")
for need in "${needs[@]:1}"; do dependencies+=("$need"); done
depends="$(printf '%s, ' "${dependencies[@]}")"
depends="${depends%, }"

name="agency_${version}_${deb_arch}.deb"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/DEBIAN" "$stage/usr/bin" "$stage/usr/share/doc/agency" "$out"
install -m 755 "$binary" "$stage/usr/bin/agency"
strip --strip-unneeded "$stage/usr/bin/agency"
for file in docs/install.md docs/install.ja.md LICENSE-MIT LICENSE-APACHE; do
  install -m 644 "$file" "$stage/usr/share/doc/agency/$(basename "$file")"
done
install -m 755 scripts/systemd-unit.sh "$stage/usr/share/doc/agency/systemd-unit.sh"
cat > "$stage/usr/share/doc/agency/copyright" <<'EOF'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: Agency

Files: *
Copyright: 2026 Agency Project contributors
License: MIT OR Apache-2.0
Comment: The full license texts are in LICENSE-MIT and LICENSE-APACHE.
EOF
cat > "$stage/DEBIAN/control" <<EOF
Package: agency
Version: $version
Architecture: $deb_arch
Maintainer: Agency Project <agency@agency.invalid>
Section: utils
Priority: optional
Depends: $depends
Description: Agency node
Description: node of a self-hosted organization of AI agents and people
 It serves an account's clients and runs hosted teams.
EOF
{
  printf 'agency (%s) unstable; urgency=medium\n\n' "$version"
  printf '  * Release %s. The changes are in CHANGELOG.md of the repository.\n\n' "$version"
  printf ' -- Agency Project <agency@agency.invalid>  %s\n' "$(git log -1 --format=%cD)"
} | gzip -9n > "$stage/usr/share/doc/agency/changelog.gz"
dpkg-deb -Zxz --root-owner-group --build "$stage" "$out/$name" >/dev/null
if command -v sha256sum >/dev/null; then
  (cd "$out" && sha256sum "$name" > "$name.sha256")
else
  (cd "$out" && shasum -a 256 "$name" > "$name.sha256")
fi
echo "Wrote $out/$name"
