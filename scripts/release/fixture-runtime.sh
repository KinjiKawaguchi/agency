#!/bin/sh
# A team runtime for the data snapshot of a release (snapshot-data.sh): speaks agency-runtime/1 with one
# request, r-1, that waits until a decision reaches it. It reads no harness and keeps its one fact in
# $SCRUM_STATE_DIR. The node runs it as `<this script> runtime`.
request="$(cat)"
org="$(printf '%s' "$request" | sed -n 's/.*"org":"\([^"]*\)".*/\1/p')"
team="$(printf '%s' "$request" | sed -n 's/.*"team":"\([^"]*\)".*/\1/p')"
decided="$SCRUM_STATE_DIR/decided"

case "$request" in
  *'"op":"describe"'*)
    echo '{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide"],"label":"Fixture"}}'
    ;;
  *'"op":"decide"'*)
    : > "$decided"
    echo '{"result":{}}'
    ;;
  *'"op":"view"'*)
    requests='[{"id":"r-1","kind":"question","title":"Ship it?","context":"","from":"agent","createdAt":"2026-11-24T12:00:00Z","blocks":[],"options":[{"key":"yes","text":"Yes","isRecommended":true}],"answerWith":{"kind":"answer","target":"r-1"}}]'
    [ ! -e "$decided" ] || requests='[]'
    echo "{\"result\":{\"org\":\"$org\",\"team\":\"$team\",\"label\":\"App\",\"generatedAt\":\"2026-11-24T12:00:00Z\",\"people\":[],\"requests\":$requests,\"work\":[],\"epics\":[]}}"
    ;;
  *)
    echo '{"error":{"status":400,"message":"unknown op"}}'
    exit 1
    ;;
esac
