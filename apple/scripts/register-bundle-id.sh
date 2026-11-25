#!/usr/bin/env bash
# Registers the app's bundle ID in the developer account and enables the capabilities the app uses
# (Push Notifications, Associated Domains) for it, through the App Store Connect API. Safe to run
# again: it only creates what is missing.
#
#   ASC_KEY_PATH=...  ASC_KEY_ID=...  ASC_ISSUER_ID=...  apple/scripts/register-bundle-id.sh
#
# The bundle ID comes from apple/Local.xcconfig. The key settings come from the environment only
# (see docs/apple.md). Nothing secret is printed.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
APPLE_DIR=$(cd "$SCRIPT_DIR/.." && pwd)
REPOSITORY_ROOT=$(cd "$APPLE_DIR/.." && pwd)
API_ROOT="${ASC_API_ROOT:-https://api.appstoreconnect.apple.com}"
APP_NAME="${ASC_APP_NAME:-Agency}"

# shellcheck source=lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"
# shellcheck source=lib/asc-jwt.sh
. "$SCRIPT_DIR/lib/asc-jwt.sh"

load_local_config "$APPLE_DIR"
load_asc_credentials "$REPOSITORY_ROOT"
for tool in curl jq openssl perl; do require_command "$tool"; done
exit_if_problems

WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT
RESPONSE="$WORK_DIR/response.json"

TOKEN=$(asc_jwt "$ASC_KEY_PATH" "$ASC_KEY_ID" "$ASC_ISSUER_ID") ||
  die "cannot sign the API token: is ASC_KEY_PATH an App Store Connect API key (.p8)?"

# asc_api METHOD PATH [JSON_BODY]
# Leaves the answer in $RESPONSE and stops on any status other than 2xx. The token goes to curl on
# its standard input, so it is not in the process list.
asc_api() {
  local method="$1" path="$2" body="${3:-}" status
  local arguments=(-sS -o "$RESPONSE" -w '%{http_code}' -X "$method" -H 'Content-Type: application/json')
  if [ -n "$body" ]; then
    arguments+=(-d "$body")
  fi
  status=$(printf 'header = "Authorization: Bearer %s"\n' "$TOKEN" |
    curl -K - "${arguments[@]}" "$API_ROOT$path") || die "cannot reach the App Store Connect API"
  case "$status" in
    2??) return 0 ;;
  esac
  printf 'error: the App Store Connect API answered HTTP %s to %s %s\n' "$status" "$method" "${path%%\?*}" >&2
  jq -r '.errors[]? | "  \(.title): \(.detail)"' "$RESPONSE" >&2 2> /dev/null || true
  exit 1
}

find_bundle_id() {
  asc_api GET "/v1/bundleIds?filter%5Bidentifier%5D=$BUNDLE_ID&filter%5Bplatform%5D=IOS&limit=200"
  # The filter also matches longer identifiers, so compare exactly.
  jq -r --arg identifier "$BUNDLE_ID" \
    '[.data[]? | select(.attributes.identifier == $identifier)][0].id // empty' "$RESPONSE"
}

create_bundle_id() {
  local body
  body=$(jq -cn --arg identifier "$BUNDLE_ID" --arg name "$APP_NAME" \
    '{data: {type: "bundleIds", attributes: {identifier: $identifier, name: $name, platform: "IOS"}}}')
  asc_api POST /v1/bundleIds "$body"
  jq -r '.data.id // empty' "$RESPONSE"
}

# The capability types the bundle ID already has, as a JSON array.
list_capability_types() {
  asc_api GET "/v1/bundleIds/$1/bundleIdCapabilities"
  jq -c '[.data[]?.attributes.capabilityType]' "$RESPONSE"
}

# ensure_capability BUNDLE_RESOURCE_ID CURRENT_TYPES TYPE LABEL
ensure_capability() {
  if printf '%s' "$2" | jq -e --arg type "$3" 'index($type)' > /dev/null; then
    printf '%s is already enabled.\n' "$4"
    return
  fi
  local body
  body=$(jq -cn --arg id "$1" --arg type "$3" '{data: {type: "bundleIdCapabilities",
    attributes: {capabilityType: $type},
    relationships: {bundleId: {data: {type: "bundleIds", id: $id}}}}}')
  asc_api POST /v1/bundleIdCapabilities "$body"
  printf 'Enabled %s.\n' "$4"
}

resource_id=$(find_bundle_id)
if [ -n "$resource_id" ]; then
  printf 'Bundle ID %s is already registered.\n' "$BUNDLE_ID"
else
  resource_id=$(create_bundle_id)
  [ -n "$resource_id" ] || die "the API created the bundle ID but did not say which one"
  printf 'Registered bundle ID %s.\n' "$BUNDLE_ID"
fi

capability_types=$(list_capability_types "$resource_id")
ensure_capability "$resource_id" "$capability_types" PUSH_NOTIFICATIONS "Push Notifications"
ensure_capability "$resource_id" "$capability_types" ASSOCIATED_DOMAINS "Associated Domains"
