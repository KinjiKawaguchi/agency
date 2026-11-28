#!/usr/bin/env bash
# Fails when a pull request changes or removes a test vector that is already on the base branch, or a file
# under testdata/releases/. docs/compatibility.md says every existing vector stays byte-identical, so that a
# signature made over the old bytes stays valid. The test in agency-proto compares the file with what the
# code generates, so regenerating the file makes it pass; this check compares the file with the base branch.
#
#   scripts/check-vectors-append-only.sh [<base ref> [<head ref>]]     (default: origin/main and HEAD)
#
# - testdata/vectors.json: each top-level entry must keep its value. An entry that is a list (`canonical`)
#   is compared item by item, so an item added at the end passes, and one changed, moved or removed fails.
# - testdata/releases/: a file that exists on the base branch must be there, unchanged.
# - A correction of a vector's own mistake is allowed by a line `<entry>: <reason>` in
#   testdata/vectors-changed.txt (`#` starts a comment). Only a line that the pull request adds counts, so
#   a correction allows that one change and not every later one. A file under testdata/releases/ is named by
#   its path.
set -euo pipefail
cd "$(dirname "$0")/.."

base_ref="${1:-origin/main}"
head_ref="${2:-HEAD}"
vectors=testdata/vectors.json
releases=testdata/releases
corrections=testdata/vectors-changed.txt

base="$(git merge-base "$base_ref" "$head_ref")" || {
  echo "Cannot find the merge base of $base_ref and $head_ref; check out the full history (fetch-depth: 0)." >&2
  exit 2
}

# A file's content at a ref, or nothing when it is not there.
content() { git show "$1:$2" 2>/dev/null || true; }
exists() { git cat-file -e "$1:$2" 2>/dev/null; }

# The entries a corrections file names, one per line, sorted.
listed() {
  content "$1" "$corrections" | sed -e '/^[[:space:]]*\(#.*\)\{0,1\}$/d' -e 's/:.*//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' | sort -u
}

# Lines of the corrections file that do not read `<entry>: <reason>`.
malformed_corrections() {
  content "$head_ref" "$corrections" | grep -vE '^[[:space:]]*(#.*)?$' | grep -vE '^[[:space:]]*[^:[:space:]][^:]*:[[:space:]]*[^[:space:]]' || true
}

# Entries of the vectors file that were removed or changed: `<removed|changed>TAB<entry>TAB<name>`.
vector_changes() {
  exists "$base" "$vectors" || return 0
  jq -n -r --slurpfile before <(content "$base" "$vectors") --slurpfile after <(content "$head_ref" "$vectors") '
    def entries:
      to_entries[] | .key as $name
      | if (.value | type) == "array"
        then .value | to_entries[] | {id: "\($name)[\(.key)]", label: (.value.name? // ""), value: .value}
        else {id: $name, label: "", value: .value} end;
    (reduce (($after[0] // {}) | entries) as $e ({}; .[$e.id] = $e.value)) as $now
    | $before[0] | entries
    | . as $entry
    | (if ($now | has($entry.id) | not) then "removed"
       elif $now[$entry.id] != $entry.value then "changed"
       else empty end) as $what
    | [$what, $entry.id, $entry.label] | @tsv'
}

# Files under testdata/releases/ of the base branch that are gone or different: `<removed|changed>TAB<path>`.
release_changes() {
  git ls-tree -r --name-only "$base" -- "$releases" | while IFS= read -r path; do
    if ! exists "$head_ref" "$path"; then
      printf 'removed\t%s\t\n' "$path"
    elif [ "$(git rev-parse "$base:$path")" != "$(git rev-parse "$head_ref:$path")" ]; then
      printf 'changed\t%s\t\n' "$path"
    fi
  done
}

bad_lines="$(malformed_corrections)"
if [ -n "$bad_lines" ]; then
  echo "$corrections needs one line per correction, '<entry>: <reason>' with a reason:" >&2
  printf '  %s\n' "$bad_lines" >&2
  exit 1
fi

allowed="$(comm -13 <(listed "$base") <(listed "$head_ref"))"
changes="$(vector_changes)"
changes="$(printf '%s\n%s\n' "$changes" "$(release_changes)" | sed '/^$/d')"

failures=0
while IFS=$'\t' read -r what entry name; do
  [ -n "$what" ] || continue
  if printf '%s\n' "$allowed" | grep -qxF -- "$entry"; then
    echo "$entry was $what, as $corrections allows."
    continue
  fi
  failures=$((failures + 1))
  echo "Entry '$entry'${name:+ ($name)} was $what." >&2
done <<< "$changes"

if [ "$failures" -gt 0 ]; then
  cat >&2 <<EOF

Existing test vectors and the files under $releases/ never change: a change to what they pin
would make signatures that exist already stop being valid (docs/compatibility.md, "Signed data").
Add new vectors at the end instead. If the vector itself was a mistake, say so in $corrections
with a line '<entry>: <reason>' (the entry as named above), and the pull request's reviewers decide.
EOF
  exit 1
fi
echo "No existing vector or release file changed."
