#!/usr/bin/env bash
# Fails when the release workflow no longer makes what a release promises: the SBOMs, and the signed
# checksums over them and the builds (docs/releasing.md, "Checking a release"). Removing a step of
# release.yml breaks nothing that runs on a pull request, so a release would simply go out without it.
#
#   scripts/ci/check-release-workflow.sh [repository root]
#
# It reads the workflow files as text: a job is the lines from its two-space-indented name to the next one.
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/../.." && pwd)}"
release="$root/.github/workflows/release.yml"
caller="$root/.github/workflows/release-please.yml"
failures=0

# job_block <file> <job>: the lines of a job.
job_block() {
  awk -v job="$2" '$0 == "  " job ":" { inside = 1; next } inside && /^  [A-Za-z_-]+:/ { exit } inside' "$1"
}
# require <description> <job block> <fixed text>: counts a failure when the job does not hold the text.
require() {
  if ! printf '%s\n' "$2" | grep -qF -- "$3"; then
    echo "$1" >&2
    failures=$((failures + 1))
  fi
}

[ -f "$release" ] && [ -f "$caller" ] || { echo "no release workflows in $root" >&2; exit 2; }
sbom="$(job_block "$release" sbom)"
attach="$(job_block "$release" attach)"
builds="$(job_block "$caller" builds)"

require "release.yml has no job 'sbom' that runs scripts/release/sbom.sh: a release would carry no SBOM." "$sbom" "scripts/release/sbom.sh"
require "the 'sbom' job of release.yml does not upload its lists as an artifact, so the release cannot carry them." "$sbom" "actions/upload-artifact"
require "the 'attach' job of release.yml does not wait for the 'sbom' job (needs), so the release could go out without the SBOMs." "$attach" "needs: [linux, sbom]"
require "the 'attach' job of release.yml does not write the checksums (scripts/release/checksums.sh)." "$attach" "scripts/release/checksums.sh"
require "the 'attach' job of release.yml does not sign SHA256SUMS (cosign sign-blob)." "$attach" "cosign sign-blob"
require "the 'attach' job of release.yml does not name the signature bundle SHA256SUMS.sigstore.json, which the release's documentation checks." "$attach" "--bundle dist/SHA256SUMS.sigstore.json"
require "the 'attach' job of release.yml has no id-token: write, which keyless signing needs." "$attach" "id-token: write"
# shellcheck disable=SC2016  # $TAG is the text of the workflow
require "the 'attach' job of release.yml does not upload dist/*, so the SBOMs and the signature would stay out of the release." "$attach" 'gh release upload "$TAG" dist/*'
require "the 'builds' job of release-please.yml does not give id-token: write to the release workflow it calls; the call would be refused at the first release." "$builds" "id-token: write"

if [ "$failures" -gt 0 ]; then
  echo "The release workflow must make the SBOMs and the signed checksums (docs/releasing.md)." >&2
  exit 1
fi
echo "The release workflow makes the SBOMs and the signed checksums."
