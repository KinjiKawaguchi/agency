#!/usr/bin/env bash
# Regenerates the TypeScript types in bindings/ from the Rust types and fails if they differ from the committed ones.
set -euo pipefail
cd "$(dirname "$0")/.."
rm -rf bindings
cargo test --quiet -p agency-proto export_bindings >/dev/null
# Compares with the index, so it also works before the first commit of a new type.
changed="$(git diff --name-status -- bindings)"
untracked="$(git ls-files --others --exclude-standard -- bindings)"
if [ -n "$changed$untracked" ]; then
  echo "bindings/ is out of date. Run scripts/check-bindings.sh and commit the result:" >&2
  printf '%s\n' "$changed" "$untracked" | sed '/^$/d' >&2
  exit 1
fi
