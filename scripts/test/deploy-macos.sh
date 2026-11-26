#!/bin/bash
# Tests scripts/deploy-macos.sh against stand-ins for launchctl and curl (scripts/test/deploy-fakes): a
# binary is "good" when its text has GOOD in it (the stand-in node answers), and "crashing" when it has
# CRASH in it (the job restarts again and again).
set -euo pipefail
cd "$(dirname "$0")/../.."
deploy_script="$PWD/scripts/deploy-macos.sh"
fakes="$PWD/scripts/test/deploy-fakes"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }
expect() { # expect <description> <actual> <expected>
  [ "$2" = "$3" ] || fail "$1: got '$2', wanted '$3'"
}
kickstarts() {
  if [ -f "$FAKE_STATE/kickstarts" ]; then wc -l < "$FAKE_STATE/kickstarts" | tr -d ' '; else echo 0; fi
}

# A fresh scenario: an installed binary with text $1 (none when empty), a new one with text $2.
scenario() {
  rm -rf "$work/s"; mkdir -p "$work/s/bin" "$work/s/data" "$work/s/state"
  FAKE_STATE="$work/s/state"; FAKE_AGENCY="$work/s/bin/agency"; export FAKE_STATE FAKE_AGENCY
  unset FAKE_NOT_LOADED
  echo 1000 > "$FAKE_STATE/pid"
  echo '{ "url": "http://127.0.0.1:4799", "listen": "127.0.0.1:4799" }' > "$work/s/data/config.json"
  echo "$work/s/data/config.json" > "$work/s/data-listing"
  if [ -n "$1" ]; then printf '#!/bin/sh\n# %s\n' "$1" > "$FAKE_AGENCY"; chmod +x "$FAKE_AGENCY"; fi
  printf '#!/bin/sh\n# %s\n' "$2" > "$work/s/new"; chmod +x "$work/s/new"
}
deploy() { # deploy [extra options]; sets status and output
  set +e
  output="$(PATH="$fakes:$PATH" "$deploy_script" --agency "$FAKE_AGENCY" --dir "$work/s/data" --label test.agency \
    --binary "$work/s/new" --skip-preflight --timeout 3 --settle 1 "$@" 2>&1)"
  status=$?
  set -e
}
installed() { sed -n 's/^# //p' "$FAKE_AGENCY"; }
previous() { sed -n 's/^# //p' "$FAKE_AGENCY.previous" 2>/dev/null || true; }

scenario "GOOD v1" "GOOD v2"
deploy
expect "a good deploy exits 0" "$status" 0
expect "the new binary is installed" "$(installed)" "GOOD v2"
expect "the old one is kept as the previous" "$(previous)" "GOOD v1"
expect "the job is restarted once" "$(kickstarts)" 1
expect "the node is asked where config.json says it listens" "$(sort -u "$FAKE_STATE/urls")" "http://127.0.0.1:4799/api/me"
expect "only the data directory's own file is there" "$(ls "$work/s/data")" "config.json"
[ ! -e "$FAKE_AGENCY.deploy-lock" ] || fail "the lock was left behind"
[ "$(find "$work/s/bin" -name '*.new.*' -o -name '*.deploy.*' | wc -l | tr -d ' ')" = 0 ] || fail "temporary files were left behind"

# The build goes where cargo puts it (CARGO_TARGET_DIR, which the team's build wrapper sets), not only
# where it usually does.
scenario "GOOD v1" "unused"
rm -rf "$work/repo" "$work/elsewhere"
mkdir -p "$work/repo/web"
git -C "$work/repo" init -q -b main
echo x > "$work/repo/file"
git -C "$work/repo" add -A
git -C "$work/repo" -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false commit -q -m "feat: x"
set +e
output="$(CARGO_TARGET_DIR="$work/elsewhere" PATH="$fakes:$PATH" "$deploy_script" --agency "$FAKE_AGENCY" --dir "$work/s/data" \
  --label test.agency --repo "$work/repo" --skip-preflight --timeout 3 --settle 1 2>&1)"
status=$?
set -e
expect "a build that cargo puts elsewhere is deployed" "$status" 0
expect "it is the binary cargo built that is installed" "$(installed)" "GOOD built"
[ ! -e "$work/repo/target" ] || fail "the binary was looked for in the repository's target directory"
expect "the web client was built first" "$(cat "$FAKE_STATE/npm")" "$(printf 'ci\nrun build')"

scenario "GOOD v1" "BAD v2"
deploy
expect "a node that does not answer exits 1" "$status" 1
expect "the previous binary is back" "$(installed)" "GOOD v1"
expect "it was restarted for the new binary and again for the old one" "$(kickstarts)" 2
case "$output" in *"did not answer at http://127.0.0.1:4799 within 3 seconds"*"the previous binary is back and the node answers"*) ;; *) fail "the output does not say what happened: $output" ;; esac

scenario "GOOD v1" "GOOD CRASH v2"
deploy
expect "a node that answers and then restarts again is not a good one" "$status" 1
expect "and the previous binary is back" "$(installed)" "GOOD v1"
case "$output" in *"stopped or restarted again"*) ;; *) fail "the output does not say it restarted again: $output" ;; esac

scenario "BAD v1" "BAD v2"
deploy
expect "when the previous binary does not answer either, exit 2" "$status" 2
case "$output" in *"look at the node now"*) ;; *) fail "the output does not tell to look at the node: $output" ;; esac

scenario "GOOD v1" "GOOD v2"
# The old process keeps answering, so an answer alone does not show that the new binary runs.
FAKE_NO_RESTART=1 deploy
expect "a restart that did not start a new process is a failure (and so is the roll back, which restarts nothing either)" "$status" 2
expect "and the previous binary is back" "$(installed)" "GOOD v1"

scenario "GOOD v1" "GOOD v2"
printf '#!/bin/sh\nexit 1\n# GOOD v2\n' > "$work/s/new"
deploy
expect "a new binary that does not run is not installed" "$status" 1
expect "the installed binary is untouched" "$(installed)" "GOOD v1"
expect "and the job is not restarted" "$(kickstarts)" 0

scenario "" "BAD v1"
deploy
expect "a first deploy that fails exits 1 and has nothing to go back to" "$status" 1
case "$output" in *"no binary before this one"*) ;; *) fail "the output does not say there was nothing to go back to: $output" ;; esac

scenario "" "GOOD v1"
deploy
expect "a first deploy exits 0" "$status" 0
expect "and has no previous binary" "$(previous)" ""

scenario "GOOD v1" "GOOD v2"
deploy
deploy --rollback --binary "$work/s/new" 2>/dev/null || true
expect "--rollback exits 0" "$status" 0
expect "--rollback installs the previous binary" "$(installed)" "GOOD v1"
expect "--rollback keeps the replaced one as the previous" "$(previous)" "GOOD v2"

scenario "GOOD v1" "GOOD v2"
FAKE_NOT_LOADED=1 deploy
expect "a job that is not loaded stops the deploy" "$status" 1
expect "before anything is installed" "$(installed)" "GOOD v1"
expect "or restarted" "$(kickstarts)" 0

scenario "GOOD v1" "GOOD v2"
mkdir "$FAKE_AGENCY.deploy-lock"
deploy
expect "a deploy that is already running stops this one" "$status" 1
expect "and nothing is installed" "$(installed)" "GOOD v1"
[ -d "$FAKE_AGENCY.deploy-lock" ] || fail "the other deploy's lock was removed"

scenario "GOOD v1" "GOOD v2"
deploy --url http://127.0.0.1:4777
expect "--url is used as given" "$(sort -u "$FAKE_STATE/urls")" "http://127.0.0.1:4777/api/me"

[ "$failures" -eq 0 ] && echo "deploy-macos: ok"
exit "$failures"
