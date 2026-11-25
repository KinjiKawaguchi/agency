#!/usr/bin/env bash
# Archives the app and sends it to App Store Connect, where it becomes a TestFlight build.
#
#   ASC_KEY_PATH=...  ASC_KEY_ID=...  ASC_ISSUER_ID=...  apple/scripts/testflight.sh
#
# Needs apple/Local.xcconfig (team and bundle ID) and an app record for that bundle ID. The key
# settings come from the environment only (see docs/apple.md). Nothing secret is printed. The build
# number is the current UTC time, so every run is higher than the one before; BUILD_NUMBER overrides it.
# The archive and this project's DerivedData are deleted at the end, also after a failure.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
APPLE_DIR=$(cd "$SCRIPT_DIR/.." && pwd)
REPOSITORY_ROOT=$(cd "$APPLE_DIR/.." && pwd)
BUILD_DIR="$APPLE_DIR/build"
DERIVED_DATA_ROOT="${DERIVED_DATA_ROOT:-$HOME/Library/Developer/Xcode/DerivedData}"

# shellcheck source=lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"

load_local_config "$APPLE_DIR"
load_asc_credentials "$REPOSITORY_ROOT"
for tool in xcodegen xcodebuild; do require_command "$tool"; done
BUILD_NUMBER="${BUILD_NUMBER:-$(date -u +%Y%m%d%H%M)}"
if ! printf '%s' "$BUILD_NUMBER" | grep -Eq '^[0-9]+(\.[0-9]+){0,2}$'; then
  problem "BUILD_NUMBER must be numbers separated by dots, for example 202611251830"
fi
exit_if_problems

# Xcode keeps one folder per project, named after it with 28 letters of hash.
remove_derived_data() {
  local folder
  for folder in "$DERIVED_DATA_ROOT"/Agency-????????????????????????????; do
    if [ -d "$folder" ]; then
      rm -rf "$folder"
    fi
  done
}

cleanup() {
  rm -rf "$BUILD_DIR"
  remove_derived_data
}
trap cleanup EXIT

cd "$APPLE_DIR"
rm -rf "$BUILD_DIR"

AUTHENTICATION=(
  -allowProvisioningUpdates
  -authenticationKeyPath "$ASC_KEY_PATH"
  -authenticationKeyID "$ASC_KEY_ID"
  -authenticationKeyIssuerID "$ASC_ISSUER_ID"
)

printf 'Generating the Xcode project.\n'
xcodegen generate --quiet || die "xcodegen failed"

printf 'Archiving build %s.\n' "$BUILD_NUMBER"
xcodebuild archive -quiet \
  -project Agency.xcodeproj -scheme Agency -configuration Release \
  -destination 'generic/platform=iOS' \
  -archivePath "$BUILD_DIR/Agency.xcarchive" \
  -derivedDataPath "$BUILD_DIR/DerivedData" \
  CURRENT_PROJECT_VERSION="$BUILD_NUMBER" \
  "${AUTHENTICATION[@]}" || die "the archive failed"

printf 'Sending build %s to App Store Connect.\n' "$BUILD_NUMBER"
xcodebuild -exportArchive -quiet \
  -archivePath "$BUILD_DIR/Agency.xcarchive" \
  -exportOptionsPlist ExportOptions.plist \
  -exportPath "$BUILD_DIR/export" \
  "${AUTHENTICATION[@]}" || die "the export or the upload failed"

printf 'Done: build %s is uploaded. It shows up in TestFlight after App Store Connect has processed it.\n' "$BUILD_NUMBER"
