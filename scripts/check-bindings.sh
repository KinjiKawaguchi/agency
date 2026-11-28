#!/usr/bin/env bash
# Regenerates the TypeScript types in bindings/ from the Rust types and fails if they differ from the committed ones.
set -euo pipefail
cd "$(dirname "$0")/.."
# The generated files are deleted, not the directory. On NFS a file that another process holds open is renamed to
# .nfsXXXX when it is removed, so the directory cannot be removed and the export that follows writes nothing.
mkdir -p bindings
find bindings -type f -name '*.ts' -delete
cargo test --quiet -p agency-proto export_bindings >/dev/null
# Compares with the index, so it also works before the first commit of a new type.
changed="$(git diff --name-status -- bindings)"
untracked="$(git ls-files --others --exclude-standard -- bindings | grep -v '/\.nfs[^/]*$' || true)"
if [ -n "$changed$untracked" ]; then
  echo "bindings/ is out of date. Run scripts/check-bindings.sh and commit the result:" >&2
  printf '%s\n' "$changed" "$untracked" | sed '/^$/d' >&2
  exit 1
fi
