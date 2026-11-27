#!/usr/bin/env bash
# Fails when a pull request's title is not a Conventional Commit, or when a breaking change does not name the
# boundary it breaks. Releases are made from the titles of merged pull requests (docs/releasing.md): a title
# that release-please cannot read gives no release and no changelog line, and nobody notices.
#
#   PR_TITLE='feat(node): ...' PR_BODY='...' [PR_AUTHOR=login] scripts/ci/check-pr-title.sh
#
# The title and the body come in the environment, never on a command line or in a shell expansion, because
# they are text anyone can write.
#
# - Title: `<type>(<scope>)!: <description>`, the scope and the `!` optional, at most 70 characters (counted
#   as characters, not bytes). A title that GitHub's revert button writes, `Revert "<such a title>"`, is let
#   through (the glossary counts it apart), and the length is counted without the `Revert` around it. A
#   bot's title (a dependency update) is not held to the length.
# - Breaking: a `!` in the title, or a body line `BREAKING CHANGE: ...` (or `BREAKING-CHANGE:`), calls for a
#   body line `BREAKING CHANGE: <boundary>: <what changes>`, and the boundary is one of the contracts of
#   docs/compatibility.md or `data`. release-please copies that line into the changelog.
set -euo pipefail
# The length is in characters: a title in Japanese must not count three bytes for each one.
export LC_ALL=C.UTF-8

title="${PR_TITLE:?PR_TITLE is not set}"
body="${PR_BODY:-}"
author="${PR_AUTHOR:-}"

types='feat|fix|perf|refactor|docs|test|build|ci|style|chore|revert'
boundaries='federation|signed text|team runtime|client API|data'
max_length=70

errors=()
add_error() { errors+=("$1"); }

# The title that is held to the rules: a revert's own title is the one inside `Revert "..."`.
checked="$title"
if [[ "$title" =~ ^Revert\ \"(.+)\"$ ]]; then
  checked="${BASH_REMATCH[1]}"
fi

conventional="^($types)(\\(([a-z0-9][a-z0-9._/-]*)\\))?(!)?: [^[:space:]].*$"
is_breaking=false
if [[ ! "$checked" =~ $conventional ]]; then
  add_error "The title is not a Conventional Commit: '<type>(<scope>)!: <description>', where the type is one of ${types//|/, } and the scope and the '!' are optional."
else
  [ -n "${BASH_REMATCH[4]}" ] && is_breaking=true
  if [ "${#checked}" -gt "$max_length" ] && [[ "$author" != *"[bot]" ]]; then
    add_error "The title is ${#checked} characters long; the limit is $max_length."
  fi
fi

breaking_lines="$(printf '%s\n' "$body" | grep -E '^BREAKING[ -]CHANGE:' || true)"
[ -n "$breaking_lines" ] && is_breaking=true

if [ "$is_breaking" = true ]; then
  named="$(printf '%s\n' "$body" | grep -ciE "^BREAKING[ -]CHANGE: ($boundaries): [^[:space:]]" || true)"
  if [ "$named" -eq 0 ]; then
    add_error "A breaking change names its boundary in the body, on a line of its own: 'BREAKING CHANGE: <boundary>: <what changes>', where the boundary is one of ${boundaries//|/, } (docs/compatibility.md)."
  fi
fi

if [ "${#errors[@]}" -gt 0 ]; then
  echo "Pull request title: $title" >&2
  for message in "${errors[@]}"; do
    echo "::error title=Pull request title::$message"
    echo "$message" >&2
  done
  exit 1
fi
echo "The title and the breaking-change footer are fine."
