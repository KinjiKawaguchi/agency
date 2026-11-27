#!/bin/bash
# Tests scripts/check-vectors-append-only.sh in a throwaway repository: a base branch with a vectors file and a
# release file, and a topic branch that changes them in one way each time.
#
#   scripts/test/vectors-append-only.sh
#
# CHECK_SCRIPT names the script to test instead of the real one, so that a mutated copy can be tried.
set -euo pipefail
cd "$(dirname "$0")/../.."
check_script="${CHECK_SCRIPT:-$PWD/scripts/check-vectors-append-only.sh}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }
expect() { [ "$2" = "$3" ] || fail "$1: got '$2', wanted '$3'"; }

base_vectors='{
  "canonical": [{"name": "first", "canonical": "{}"}, {"name": "second", "canonical": "{\"a\":1}"}],
  "envelope": {"canonical": "{\"x\":1}", "signature": "aa"},
  "org": {"canonical": "{\"o\":1}", "signature": "bb"},
  "seed": "0909"
}'

# fresh_repo: a repository whose main holds the vectors, a release file, and an old correction.
fresh_repo() {
  repo="$work/repo"
  rm -rf "$repo"
  mkdir -p "$repo/scripts" "$repo/testdata/releases"
  cp "$check_script" "$repo/scripts/check-vectors-append-only.sh"
  cd "$repo"
  git init -q -b main
  git config user.email t@example.test
  git config user.name t
  git config commit.gpgsign false
  printf '%s\n' "$base_vectors" > testdata/vectors.json
  printf 'frozen\n' > testdata/releases/0.1.0.json
  printf '# corrections\nseed: the seed was written with a typo\n' > testdata/vectors-changed.txt
  git add -A && git commit -qm "chore: base"
  git switch -q -c topic
}
commit_topic() { git add -A && git commit -qm "chore: topic"; }

# vectors <jq filter>: rewrites the vectors file with a jq filter.
vectors() { jq "$1" testdata/vectors.json > "$work/v.json" && mv "$work/v.json" testdata/vectors.json; }

# check: runs the script against main, and sets status and output.
check() {
  set +e
  output="$(scripts/check-vectors-append-only.sh main 2>&1)"
  status=$?
  set -e
}
passes() { check; expect "$1: exit status" "$status" 0; }
fails() { # fails <what> <text the output must hold>
  check
  expect "$1: exit status" "$status" 1
  case "$output" in *"$2"*) ;; *) fail "$1: the output does not hold '$2': $output" ;; esac
}

fresh_repo; git commit -q --allow-empty -m "chore: nothing"
passes "an unchanged file passes"

fresh_repo; vectors '.newKind = {"canonical": "{}", "signature": "cc"}'; commit_topic
passes "a new entry passes"

fresh_repo; vectors '.canonical += [{"name": "third", "canonical": "{\"b\":2}"}]'; commit_topic
passes "an item added at the end of a list passes"

fresh_repo; vectors '.envelope.signature = "ff"'; commit_topic
fails "a changed entry fails" "Entry 'envelope' was changed"
case "$output" in *"signatures that exist already stop being valid"*) ;; *) fail "the output does not say old signatures would stop being valid: $output" ;; esac

fresh_repo; vectors 'del(.org)'; commit_topic
fails "a removed entry fails" "Entry 'org' was removed"

fresh_repo; vectors '.canonical[1].canonical = "{}"'; commit_topic
fails "a changed item of a list fails, and is named" "Entry 'canonical[1]' (second) was changed"

fresh_repo; vectors '.canonical |= [.[1], .[0]]'; commit_topic
fails "a reordered list fails" "was changed"

fresh_repo; vectors '.canonical |= .[:1]'; commit_topic
fails "a removed item of a list fails" "Entry 'canonical[1]' (second) was removed"

fresh_repo; vectors '.org.signature = "ff" | .envelope.signature = "ee"'; commit_topic
fails "every change is named, not only the first" "Entry 'org' was changed"
case "$output" in *"Entry 'envelope' was changed"*) ;; *) fail "the second change is not named: $output" ;; esac

fresh_repo; git rm -q testdata/vectors.json; commit_topic
fails "a deleted vectors file fails" "Entry 'org' was removed"

fresh_repo; git switch -q main; git rm -q testdata/vectors.json; git commit -qm "chore: no vectors"; git switch -q topic
git rebase -q main; printf '%s\n' "$base_vectors" > testdata/vectors.json; commit_topic
passes "a vectors file that the base does not have passes"

# Corrections.
fresh_repo; vectors '.envelope.signature = "ff"'; printf 'envelope: the signature was made over the wrong body\n' >> testdata/vectors-changed.txt; commit_topic
passes "a listed correction passes"
case "$output" in *"envelope was changed, as testdata/vectors-changed.txt allows"*) ;; *) fail "the output does not say the correction allowed it: $output" ;; esac

fresh_repo; vectors '.envelope.signature = "ff" | .org.signature = "ee"'; printf 'envelope: a typo\n' >> testdata/vectors-changed.txt; commit_topic
fails "a correction allows the entry it names and no other" "Entry 'org' was changed"

fresh_repo; vectors '.seed = "0a0a"'; commit_topic
fails "a correction that main already lists allows nothing new" "Entry 'seed' was changed"

fresh_repo; vectors '.canonical[1].canonical = "{}"'; printf 'canonical[1]: the second case was wrong\n' >> testdata/vectors-changed.txt; commit_topic
passes "an item of a list is corrected by its index"

fresh_repo; vectors '.envelope.signature = "ff"'; printf 'envelope:\n' >> testdata/vectors-changed.txt; commit_topic
fails "a correction with no reason fails" "needs one line per correction"

fresh_repo; vectors '.envelope.signature = "ff"'; printf 'envelope\n' >> testdata/vectors-changed.txt; commit_topic
fails "a line with no colon fails" "needs one line per correction"

# Releases.
fresh_repo; printf 'another\n' > testdata/releases/0.2.0.json; commit_topic
passes "a new release file passes"

fresh_repo; printf 'edited\n' > testdata/releases/0.1.0.json; commit_topic
fails "a changed release file fails" "Entry 'testdata/releases/0.1.0.json' was changed"

fresh_repo; git rm -q testdata/releases/0.1.0.json; commit_topic
fails "a removed release file fails" "Entry 'testdata/releases/0.1.0.json' was removed"

fresh_repo; printf 'edited\n' > testdata/releases/0.1.0.json; printf 'testdata/releases/0.1.0.json: it was written for the wrong release\n' >> testdata/vectors-changed.txt; commit_topic
passes "a listed correction of a release file passes"

# The merge base: main moves on after the topic branch was made, and its changes are not the topic's.
fresh_repo
git switch -q main; vectors '.envelope.signature = "ff"'; git add -A; git commit -qm "chore: main changes an entry"; git switch -q topic
printf 'x\n' > unrelated.txt; commit_topic
passes "a change that is on main only is not the topic branch's"

[ "$failures" -eq 0 ] && echo "vectors-append-only: ok"
exit "$failures"
