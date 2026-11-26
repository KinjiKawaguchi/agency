#!/usr/bin/env bash
# Registers the app's bundle ID and the widget extension's (`<bundle id>.widgets`) in the developer account
# and enables the capabilities they use (the app: Push Notifications, Associated Domains, App Groups; the
# widget extension: App Groups) through the App Store Connect API. Safe to run again: it only creates what
# is missing.
#
# The API cannot create the app group itself (`group.<bundle id>`): make it once in the developer portal
# (Identifiers, App Groups), or let Xcode's automatic signing create it during the TestFlight build.
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
WIDGET_NAME="${ASC_WIDGET_NAME:-$APP_NAME Widgets}"

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
# Leaves the answer in $RESPONSE and stops on any status other than 2xx; with SOFT=1 it only returns 1
# then (the report on the Mac must not stop the script when a read is not allowed). The token goes to
# curl on its standard input, so it is not in the process list.
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
  if [ "${SOFT:-}" = 1 ]; then
    return 1
  fi
  exit 1
}

# find_bundle_id IDENTIFIER
find_bundle_id() {
  asc_api GET "/v1/bundleIds?filter%5Bidentifier%5D=$1&filter%5Bplatform%5D=IOS&limit=200"
  # The filter also matches longer identifiers, so compare exactly.
  jq -r --arg identifier "$1" \
    '[.data[]? | select(.attributes.identifier == $identifier)][0].id // empty' "$RESPONSE"
}

# create_bundle_id IDENTIFIER NAME
create_bundle_id() {
  local body
  body=$(jq -cn --arg identifier "$1" --arg name "$2" \
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

# register_bundle_id IDENTIFIER NAME CAPABILITY_TYPE:LABEL...
# Registers the identifier when it is missing and turns on the capabilities it lacks.
register_bundle_id() {
  local identifier="$1" name="$2" resource_id capability_types entry
  shift 2
  resource_id=$(find_bundle_id "$identifier")
  if [ -n "$resource_id" ]; then
    printf 'Bundle ID %s is already registered.\n' "$identifier"
  else
    resource_id=$(create_bundle_id "$identifier" "$name")
    [ -n "$resource_id" ] || die "the API created the bundle ID but did not say which one"
    printf 'Registered bundle ID %s.\n' "$identifier"
  fi
  capability_types=$(list_capability_types "$resource_id")
  for entry in "$@"; do
    ensure_capability "$resource_id" "$capability_types" "${entry%%:*}" "${entry#*:}"
  done
}

register_bundle_id "$BUNDLE_ID" "$APP_NAME" \
  "PUSH_NOTIFICATIONS:Push Notifications" "ASSOCIATED_DOMAINS:Associated Domains" "APP_GROUPS:App Groups"
register_bundle_id "$BUNDLE_ID.widgets" "$WIDGET_NAME" "APP_GROUPS:App Groups"

# The version the app is built as (MARKETING_VERSION in project.yml): the macOS version the record is given is
# that one, so no other, empty version stays on the record.
marketing_version() {
  local version
  version=$(awk -F: '/MARKETING_VERSION:/ { gsub(/[ "]/, "", $2); print $2; exit }' "$APPLE_DIR/project.yml" 2> /dev/null)
  [ -n "$version" ] || die "cannot read MARKETING_VERSION from apple/project.yml"
  printf '%s' "$version"
}

# What the Mac needs besides the bundle ID, checked here and only changed when asked: the app record may have
# no macOS platform yet. Adding it is one-way (an app with two platforms is a universal purchase for good),
# so it happens only with ADD_MACOS_PLATFORM=1; the web interface does the same ("Add Platform", macOS).
check_macos() {
  local app_id count version body
  version=$(marketing_version)
  if SOFT=1 asc_api GET "/v1/bundleIds?filter%5Bidentifier%5D=$BUNDLE_ID&filter%5Bplatform%5D=MAC_OS&limit=200"; then
    if jq -e --arg identifier "$BUNDLE_ID" '[.data[]? | select(.attributes.identifier == $identifier)] | length > 0' "$RESPONSE" > /dev/null; then
      printf 'The API lists %s as a macOS bundle ID too.\n' "$BUNDLE_ID"
    else
      printf 'The API lists no macOS bundle ID %s, only the iOS one. Whether the Mac build can sign with it is what its first archive tells; this script does not create a second record.\n' "$BUNDLE_ID"
    fi
  else
    printf 'The macOS bundle IDs could not be read with this key; skipped.\n'
  fi
  if ! SOFT=1 asc_api GET "/v1/apps?filter%5BbundleId%5D=$BUNDLE_ID&limit=200"; then
    printf 'The app record could not be read with this key, so nothing is said of its macOS platform and nothing is added.\n'
    return
  fi
  app_id=$(jq -r --arg identifier "$BUNDLE_ID" '[.data[]? | select(.attributes.bundleId == $identifier)][0].id // empty' "$RESPONSE")
  if [ -z "$app_id" ]; then
    printf 'There is no app record for %s in App Store Connect yet, so nothing can be said of its macOS platform.\n' "$BUNDLE_ID"
    return
  fi
  if ! SOFT=1 asc_api GET "/v1/apps/$app_id/appStoreVersions?filter%5Bplatform%5D=MAC_OS&limit=50"; then
    printf 'The versions of the app record could not be read with this key, so nothing is said of its macOS platform and nothing is added.\n'
    return
  fi
  # Only a version whose platform is macOS counts: the answer to the filter is not trusted to be only that.
  count=$(jq '[.data[]? | select(.attributes.platform == "MAC_OS")] | length' "$RESPONSE")
  if [ "$count" -gt 0 ]; then
    printf 'The app record already has a macOS platform.\n'
  elif [ "${ADD_MACOS_PLATFORM:-}" = 1 ]; then
    body=$(jq -cn --arg id "$app_id" --arg version "${ASC_MACOS_VERSION:-$version}" \
      '{data: {type: "appStoreVersions", attributes: {platform: "MAC_OS", versionString: $version},
      relationships: {app: {data: {type: "apps", id: $id}}}}}')
    asc_api POST /v1/appStoreVersions "$body"
    printf 'Added the macOS platform to the app record, version %s.\n' "${ASC_MACOS_VERSION:-$version}"
  else
    printf 'The app record has no macOS platform. Adding it cannot be undone: add it in App Store Connect (the app, Add Platform, macOS) or run this again with ADD_MACOS_PLATFORM=1.\n'
  fi
}

check_macos

printf 'Next: the app group group.%s must exist and be ticked for both bundle IDs. The API cannot create it.\n' "$BUNDLE_ID"
