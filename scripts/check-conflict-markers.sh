#!/usr/bin/env bash
# Fails if a tracked text file still holds Git conflict markers. A Markdown file with markers builds and
# passes every test, so nothing else catches one left by a rebase. Only the opening and closing markers
# are checked: `=======` alone is also a Markdown heading underline.
set -euo pipefail
cd "$(dirname "$0")/.."
if git grep -nIE '^(<<<<<<<|>>>>>>>)( |$)' -- . ; then
  echo "Conflict markers found in the files above; resolve them before merging." >&2
  exit 1
fi
