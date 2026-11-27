#!/bin/bash
# Tests scripts/ci/check-release-workflow.sh: the real workflow files pass, and each step it requires, taken
# out of a copy of them, fails the check and is named.
#
#   scripts/test/release-workflow.sh
#
# CHECK_SCRIPT names the script to test instead of the real one, so that a mutated copy can be tried.
set -euo pipefail
cd "$(dirname "$0")/../.."
check_script="${CHECK_SCRIPT:-$PWD/scripts/ci/check-release-workflow.sh}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }

# fresh_root: a copy of the two workflow files, in $work/root.
fresh_root() {
  rm -rf "$work/root"
  mkdir -p "$work/root/.github/workflows"
  cp .github/workflows/release.yml .github/workflows/release-please.yml "$work/root/.github/workflows/"
}
run() { status=0; output="$("$check_script" "$work/root" 2>&1)" || status=$?; }

# broken <what> <file> <sed script> <text the output must hold>: a copy changed by the sed script fails.
broken() {
  fresh_root
  sed -i "$3" "$work/root/.github/workflows/$2"
  if cmp -s ".github/workflows/$2" "$work/root/.github/workflows/$2"; then fail "$1: the change did not apply"; return; fi
  run
  [ "$status" -eq 1 ] || fail "$1: wanted a failure, got $status: $output"
  case "$output" in *"$4"*) ;; *) fail "$1: the output does not hold '$4': $output" ;; esac
}

fresh_root; run
[ "$status" -eq 0 ] || fail "the workflows as they are fail the check: $output"
fresh_root; sed -i 's|^# Release builds of the node for Linux.|# Release builds, in other words, of the node for Linux.|' "$work/root/.github/workflows/release.yml"; run
[ "$status" -eq 0 ] || fail "a change to a comment fails the check: $output"

broken "the SBOM script is not run" release.yml 's|scripts/release/sbom.sh|true|' "no job 'sbom' that runs scripts/release/sbom.sh"
broken "the SBOM job is gone" release.yml '/^  sbom:/,/^  # Attaches the builds/{/^  sbom:/d;/^  # Attaches the builds/!d}' "no job 'sbom'"
broken "the lists are not uploaded" release.yml '/^  sbom:/,/^  attach:/ s|actions/upload-artifact|actions/cache|' "does not upload its lists"
broken "an upload in the next job is not the SBOM job's" release.yml '/^  sbom:/,/^  attach:/ s|actions/upload-artifact|actions/cache|;/^  attach:/a\    # actions/upload-artifact' "does not upload its lists"
broken "attach does not wait for the SBOMs" release.yml 's|needs: \[linux, sbom\]|needs: linux|' "does not wait for the 'sbom' job"
broken "the checksums are not written" release.yml 's|scripts/release/checksums.sh|true|' "does not write the checksums"
broken "the checksums are not signed" release.yml 's|cosign sign-blob|true|' "does not sign SHA256SUMS"
broken "the bundle has another name" release.yml 's|SHA256SUMS.sigstore.json|SHA256SUMS.sig|' "SHA256SUMS.sigstore.json"
broken "attach has no id-token" release.yml '/^  attach:/,$ s|id-token: write|id-token: none|' "no id-token: write"
# shellcheck disable=SC2016  # $TAG is the text of the workflow
broken "the release does not get dist/*" release.yml 's|gh release upload "\$TAG" dist/\*|gh release upload "$TAG" dist/agency.tar.gz|' "does not upload dist/*"
broken "the caller gives no id-token" release-please.yml 's|id-token: write|id-token: none|' "does not give id-token: write"

fresh_root; rm "$work/root/.github/workflows/release.yml"; run
[ "$status" -eq 2 ] || fail "a repository with no release workflow gave $status, not 2"

[ "$failures" -eq 0 ] && echo "release-workflow: ok"
exit "$failures"
