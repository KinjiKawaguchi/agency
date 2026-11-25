#!/usr/bin/env bash
# Fails if a translated document (docs/<name>.<lang>.md) and its English source (docs/<name>.md) have
# different numbers of level-2 or level-3 headings, so that a section added to one cannot go missing in
# the other unnoticed. Lines inside fenced code blocks are not headings.
set -euo pipefail
cd "$(dirname "$0")/.."

headings() {
  awk -v level="$2" '
    /^```/ { fenced = !fenced; next }
    !fenced && index($0, level " ") == 1 { count++ }
    END { print count + 0 }
  ' "$1"
}

status=0
for translation in docs/*.*.md; do
  [ -e "$translation" ] || continue
  source="${translation%.*.md}.md"
  if [ ! -f "$source" ]; then
    echo "$translation has no English source $source" >&2
    status=1
    continue
  fi
  for level in '##' '###'; do
    expected=$(headings "$source" "$level")
    actual=$(headings "$translation" "$level")
    if [ "$expected" != "$actual" ]; then
      echo "$translation has $actual '$level' headings; $source has $expected" >&2
      status=1
    fi
  done
done
exit "$status"
