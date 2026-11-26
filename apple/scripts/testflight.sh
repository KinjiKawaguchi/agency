#!/usr/bin/env bash
# Archives the app and sends it to App Store Connect, where it becomes a TestFlight build.
#
#   ASC_KEY_PATH=...  ASC_KEY_ID=...  ASC_ISSUER_ID=...  apple/scripts/testflight.sh [--platform ios|macos]
#
# The platform is ios unless it says macos, which builds the Mac app (the AgencyMac scheme) for TestFlight
# for Mac. Both go to the same app record, because the Mac app has the iPhone app's bundle ID.
#
# Needs apple/Local.xcconfig (team and bundle ID) and an app record for that bundle ID. The key
# settings come from the environment only (see docs/apple.md). Nothing secret is printed. The build
# number is the current UTC time, so every run is higher than the one before; BUILD_NUMBER overrides it.
# The build says which source it was made from (the commit, and `git describe` against the apple-v tags,
# in its Info.plist), and each upload is recorded in apple/uploads.log: time, build number, platform, version,
# commit, and describe. A tree with uncommitted changes is built, and recorded as -dirty.
# The archive and this project's DerivedData are deleted at the end, also after a failure.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
APPLE_DIR=$(cd "$SCRIPT_DIR/.." && pwd)
REPOSITORY_ROOT=$(cd "$APPLE_DIR/.." && pwd)
BUILD_DIR="$APPLE_DIR/build"
DERIVED_DATA_ROOT="${DERIVED_DATA_ROOT:-$HOME/Library/Developer/Xcode/DerivedData}"

# shellcheck source=lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"

PLATFORM="ios"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --platform)
      [ "$#" -ge 2 ] || die "--platform needs ios or macos"
      PLATFORM="$2"
      shift 2
      ;;
    *) die "unknown argument: $1 (the only option is --platform ios|macos)" ;;
  esac
done
case "$PLATFORM" in
  ios)
    SCHEME="Agency"
    DESTINATION="generic/platform=iOS"
    ;;
  macos)
    SCHEME="AgencyMac"
    DESTINATION="generic/platform=macOS"
    ;;
  *) die "--platform must be ios or macos, not '$PLATFORM'" ;;
esac

load_local_config "$APPLE_DIR"
load_asc_credentials "$REPOSITORY_ROOT"
for tool in xcodegen xcodebuild; do require_command "$tool"; done
BUILD_NUMBER="${BUILD_NUMBER:-$(date -u +%Y%m%d%H%M)}"
if ! printf '%s' "$BUILD_NUMBER" | grep -Eq '^[0-9]+(\.[0-9]+){0,2}$'; then
  problem "BUILD_NUMBER must be numbers separated by dots, for example 202611251830"
fi
exit_if_problems

# Where the build comes from. Outside a git checkout, or without a tag, these say so.
BUILD_COMMIT=$(git -C "$REPOSITORY_ROOT" rev-parse --short=12 HEAD 2> /dev/null || echo unknown)
BUILD_DESCRIBE=$(git -C "$REPOSITORY_ROOT" describe --tags --match 'apple-v*' --long --dirty --always --abbrev=12 2> /dev/null || echo unknown)
MARKETING_VERSION=$(sed -n 's/^ *MARKETING_VERSION: "\(.*\)"/\1/p' "$APPLE_DIR/project.yml" 2> /dev/null | head -n 1 || true)
case "$BUILD_DESCRIBE" in
  *-dirty) printf 'Warning: the working tree has uncommitted changes; this build is recorded as %s.\n' "$BUILD_DESCRIBE" ;;
esac

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

printf 'Archiving build %s for %s.\n' "$BUILD_NUMBER" "$PLATFORM"
xcodebuild archive -quiet \
  -project Agency.xcodeproj -scheme "$SCHEME" -configuration Release \
  -destination "$DESTINATION" \
  -archivePath "$BUILD_DIR/Agency.xcarchive" \
  -derivedDataPath "$BUILD_DIR/DerivedData" \
  CURRENT_PROJECT_VERSION="$BUILD_NUMBER" \
  AGENCY_BUILD_COMMIT="$BUILD_COMMIT" \
  AGENCY_BUILD_DESCRIBE="$BUILD_DESCRIBE" \
  "${AUTHENTICATION[@]}" || die "the archive failed"

printf 'Sending build %s to App Store Connect.\n' "$BUILD_NUMBER"
xcodebuild -exportArchive -quiet \
  -archivePath "$BUILD_DIR/Agency.xcarchive" \
  -exportOptionsPlist ExportOptions.plist \
  -exportPath "$BUILD_DIR/export" \
  "${AUTHENTICATION[@]}" || die "the export or the upload failed"

printf '%s\tbuild %s\tplatform %s\tversion %s\tcommit %s\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$BUILD_NUMBER" "$PLATFORM" \
  "${MARKETING_VERSION:-unknown}" "$BUILD_COMMIT" "$BUILD_DESCRIBE" >> "$APPLE_DIR/uploads.log"
printf 'Done: build %s (%s) is uploaded. It shows up in TestFlight after App Store Connect has processed it.\n' "$BUILD_NUMBER" "$PLATFORM"
