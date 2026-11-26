#!/bin/bash
# Checks that a Debian package installs only the node and its documentation, then removes it cleanly.
set -euo pipefail
package="${1:?usage: $0 <package>}"
dpkg-deb --info "$package"
dpkg-deb --contents "$package"
contents="$(dpkg-deb --contents "$package")"
case "$contents" in *" ./etc/"* | *" ./lib/systemd/"* | *" ./usr/lib/systemd/"*) echo "package installs configuration or a system unit" >&2; exit 1 ;; esac
scripts="$(dpkg-deb --ctrl-tarfile "$package" | tar -t | grep -vxE '\./|\./control' || true)"
[ -z "$scripts" ] || { echo "package has maintainer scripts or other control files: $scripts" >&2; exit 1; }
sudo dpkg -i "$package"
agency --version
getent passwd agency >/dev/null && { echo "package created an agency account" >&2; exit 1; }
systemctl list-unit-files 'agency*' | grep -q agency && { echo "package installed an agency unit" >&2; exit 1; }
sudo dpkg -r agency
test -e /usr/bin/agency && { echo "binary remains after removal" >&2; exit 1; }
command -v agency >/dev/null && { echo "agency remains on PATH after removal" >&2; exit 1; }
sudo dpkg --purge agency
lintian --fail-on error "$package"
