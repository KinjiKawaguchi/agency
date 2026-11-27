#!/bin/bash
# Tests scripts/ci/check-pr-title.sh with good and bad titles and breaking-change footers, one case per line.
#
#   scripts/test/pr-title.sh
#
# LC_ALL=C is set for every case, to show that the script chooses its own locale.
# CHECK_SCRIPT names the script to test instead of the real one, so that a mutated copy can be tried.
set -euo pipefail
cd "$(dirname "$0")/../.."
check_script="${CHECK_SCRIPT:-$PWD/scripts/ci/check-pr-title.sh}"

failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }

# check <what> <ok|bad> <title> [body [author]]: runs the script, and says what went wrong when it did not
# end as wanted. Sets output.
check() {
  local what="$1" wanted="$2" status=0
  output="$(LC_ALL=C PR_TITLE="$3" PR_BODY="${4:-}" PR_AUTHOR="${5:-}" "$check_script" 2>&1)" || status=$?
  if [ "$wanted" = ok ] && [ "$status" -ne 0 ]; then fail "$what: wanted ok, got $status: $output"; fi
  if [ "$wanted" = bad ] && [ "$status" -ne 1 ]; then fail "$what: wanted a failure, got $status: $output"; fi
  return 0
}
mentions() { case "$output" in *"$2"*) ;; *) fail "$1: the output does not hold '$2': $output" ;; esac; }

# Titles.
for type in feat fix perf refactor docs test build ci style chore revert; do
  check "type $type" ok "$type: say why"
done
check "a scope" ok "feat(node): answer a decision once it is kept"
check "a scope with a dash, a dot and a slash" ok "fix(core-2.x/io): keep the order"
check "a release title" ok "chore: release 0.3.0"
check "an Apple release title" ok "chore: release apple 0.3.0"
check "a dependency title" ok "chore(deps): update dependency vitest to v3"
check "a revert that GitHub writes" ok 'Revert "feat(node): answer a decision once it is kept"'
check "no type" bad "answer a decision once it is kept"
check "a type that is not one of ours" bad "feature: answer a decision"
check "a capital letter in the type" bad "Feat: answer a decision"
check "no colon" bad "feat answer a decision"
check "no space after the colon" bad "feat:answer a decision"
check "an empty description" bad "feat: "
check "an empty scope" bad "feat(): answer a decision"
check "a scope with a space" bad "feat(my scope): answer a decision"
check "the exclamation mark before the scope" bad "feat!(node): answer a decision"
check "a revert of something that is not a Conventional Commit" bad 'Revert "answer a decision"'
check "a word with no shape" bad "WIP"
mentions "a bad title says what a good one looks like" "<type>(<scope>)!: <description>"

# Length: 70 characters at most, counted as characters; a bot's title is not held to it.
prefix="feat(node): "
long70="$prefix$(printf 'x%.0s' $(seq 1 $((70 - ${#prefix}))))"
check "70 characters" ok "$long70"
check "71 characters" bad "${long70}x"
mentions "71 characters" "The title is 71 characters long; the limit is 70."
check "a title of 70 characters in Japanese" ok "feat(web): $(printf 'あ%.0s' $(seq 1 $((70 - 11))))"
check "a title of 71 characters in Japanese" bad "feat(web): $(printf 'あ%.0s' $(seq 1 $((71 - 11))))"
check "a bot's long title" ok "$long70$(printf 'x%.0s' $(seq 1 50))" "" "renovate[bot]"
check "a person with a name like a bot's" bad "$long70$(printf 'x%.0s' $(seq 1 50))" "" "robot"
check "a revert is counted without the word Revert" ok "Revert \"$long70\""
check "a revert of a title that is too long" bad "Revert \"${long70}x\""

# Breaking changes.
for boundary in "federation" "signed text" "team runtime" "client API" "data"; do
  check "title with ! and the boundary $boundary" ok "feat(node)!: drop a route" "Why

BREAKING CHANGE: $boundary: the route is gone"
  check "footer alone with the boundary $boundary" ok "feat(node): drop a route" "BREAKING CHANGE: $boundary: the route is gone"
done
check "a footer keyword in lowercase is not the footer" bad "feat(node)!: drop a route" "breaking change: data: gone"
check "a footer keyword in capitals and lowercase is not the footer" bad "feat(node)!: drop a route" "Breaking Change: data: gone"
check "a lowercase keyword next to a good footer does not matter" ok "feat(node)!: drop a route" "breaking change: data: gone
BREAKING CHANGE: data: gone"
check "a lowercase keyword alone, without a !, is only text" ok "feat(node): drop a route" "breaking change: nothing to see"
check "the hyphenated footer" ok "feat(node): drop a route" "BREAKING-CHANGE: federation: the route is gone"
check "a boundary in other letters" ok "feat(node): drop a route" "BREAKING CHANGE: Client Api: the route is gone"
check "the footer among other lines, with CRLF" ok "fix(node)!: drop a route" $'Why\r\n\r\nBREAKING CHANGE: data: the file is gone\r\n\r\nCloses #1'
check "! with no footer" bad "feat(node)!: drop a route" "Why: it was wrong"
mentions "a missing boundary says how to name it" "BREAKING CHANGE: <boundary>: <what changes>"
check "! with an empty body" bad "feat(node)!: drop a route"
check "a footer with no boundary" bad "feat(node)!: drop a route" "BREAKING CHANGE: the route is gone"
check "a footer with a boundary that is not one" bad "feat(node)!: drop a route" "BREAKING CHANGE: the moon: the route is gone"
check "a boundary with no description" bad "feat(node)!: drop a route" "BREAKING CHANGE: federation:"
check "a boundary with only a space after it" bad "feat(node)!: drop a route" "BREAKING CHANGE: federation: "
check "a footer alone with no boundary" bad "feat(node): drop a route" "BREAKING CHANGE: the route is gone"
check "the word in the middle of a line is not a footer" ok "feat(node): drop a route" "This is not a BREAKING CHANGE: federation"
check "a footer cannot be taken for the boundary of another line" bad "feat(node)!: drop a route" "BREAKING CHANGE: nothing
see BREAKING CHANGE: federation: elsewhere"
check "a bad type is said even when the title has a !" bad "nope!: x"
mentions "a bad type with a !" "not a Conventional Commit"

# The title and the body are text from outside: they are never run.
marker="$(mktemp -u)"
check "a title with a command in it is only text" ok "feat: \$(touch $marker)\`touch $marker\`;" "\$(touch $marker)"
[ ! -e "$marker" ] || fail "the title or the body was run as a command"

[ "$failures" -eq 0 ] && echo "pr-title: ok"
exit "$failures"
