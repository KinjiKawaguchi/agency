#!/usr/bin/env bash
# Tests for the scripts in apple/scripts, with no Apple account, no network, and no Xcode:
#   apple/scripts/test/run.sh
# Needs bash, openssl, perl, and jq.
set -u

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
SCRIPTS=$(cd "$HERE/.." && pwd)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

FAILURES=0
pass() { printf '  ok    %s\n' "$1"; }
fail() { printf '  FAIL  %s\n' "$1"; FAILURES=$((FAILURES + 1)); }
check() { # description, command...
  local description="$1"
  shift
  if "$@" > /dev/null 2>&1; then pass "$description"; else fail "$description"; fi
}
check_not() {
  local description="$1"
  shift
  if "$@" > /dev/null 2>&1; then fail "$description"; else pass "$description"; fi
}
contains() { printf '%s' "$1" | grep -Fq -- "$2"; }

# shellcheck source=../lib/common.sh
. "$SCRIPTS/lib/common.sh"
# shellcheck source=../lib/asc-jwt.sh
. "$SCRIPTS/lib/asc-jwt.sh"

# ---------------------------------------------------------------- JWT

KEY="$WORK/AuthKey.p8"
PUBLIC_KEY="$WORK/public.pem"
openssl ecparam -name prime256v1 -genkey -noout 2> /dev/null | openssl pkcs8 -topk8 -nocrypt -out "$KEY"
openssl pkey -in "$KEY" -pubout -out "$PUBLIC_KEY"

decode_segment() { # a base64url segment
  printf '%s' "$1" | tr '_-' '/+' |
    perl -MMIME::Base64 -e 'local $_ = <STDIN>; $_ .= "=" x ((4 - length($_) % 4) % 4); print decode_base64($_)'
}

signature_matches() { # jwt, public key file
  local header payload signature hex
  IFS=. read -r header payload signature <<< "$1"
  hex=$(printf '%s' "$signature" | tr '_-' '/+' |
    perl -MMIME::Base64 -e 'local $_ = <STDIN>; $_ .= "=" x ((4 - length($_) % 4) % 4); print unpack("H*", decode_base64($_))')
  [ "${#hex}" -eq 128 ] || return 1
  printf 'asn1=SEQUENCE:sig\n[sig]\nr=INTEGER:0x%s\ns=INTEGER:0x%s\n' "${hex:0:64}" "${hex:64:64}" > "$WORK/sig.conf"
  openssl asn1parse -genconf "$WORK/sig.conf" -noout -out "$WORK/sig.der" || return 1
  printf '%s.%s' "$header" "$payload" | openssl dgst -sha256 -verify "$2" -signature "$WORK/sig.der" > /dev/null
}

echo "JWT"
JWT=$(asc_jwt "$KEY" ABCDE12345 11111111-2222-3333-4444-555555555555 1800000000)
IFS=. read -r JWT_HEADER JWT_PAYLOAD _ <<< "$JWT"
HEADER_JSON=$(decode_segment "$JWT_HEADER")
PAYLOAD_JSON=$(decode_segment "$JWT_PAYLOAD")
check "the header names ES256 and the key" \
  jq -e '.alg == "ES256" and .kid == "ABCDE12345" and .typ == "JWT"' <<< "$HEADER_JSON"
check "the payload names the issuer and the API, and lasts ten minutes" \
  jq -e '.iss == "11111111-2222-3333-4444-555555555555" and .aud == "appstoreconnect-v1" and .iat == 1800000000 and .exp == 1800000600' <<< "$PAYLOAD_JSON"
check "the signature verifies with the public key" signature_matches "$JWT" "$PUBLIC_KEY"
check_not "a changed payload does not verify" signature_matches "$JWT_HEADER.$(printf '{"iss":"x"}' | base64url).${JWT##*.}" "$PUBLIC_KEY"
OTHER_KEY="$WORK/other.pem"
openssl ecparam -name prime256v1 -genkey -noout 2> /dev/null | openssl pkey -pubout -out "$OTHER_KEY"
check_not "another key's public half does not verify" signature_matches "$JWT" "$OTHER_KEY"
SIGNATURES_OK=true
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  signature_matches "$(asc_jwt "$KEY" ABCDE12345 11111111-2222-3333-4444-555555555555)" "$PUBLIC_KEY" || SIGNATURES_OK=false
done
check "twenty signatures in a row verify" "$SIGNATURES_OK"
check_not "a file that is not a key gives no token" asc_jwt "$WORK/sig.conf" ABCDE12345 11111111-2222-3333-4444-555555555555

der_hex() { # r, s as hex
  printf 'asn1=SEQUENCE:sig\n[sig]\nr=INTEGER:0x%s\ns=INTEGER:0x%s\n' "$1" "$2" > "$WORK/der.conf"
  openssl asn1parse -genconf "$WORK/der.conf" -noout -out "$WORK/der.bin"
  der_signature_to_raw_hex < "$WORK/der.bin"
}
ZEROS=0000000000000000000000000000000000000000000000000000000000000000
HIGH=8000000000000000000000000000000000000000000000000000000000000001
check "a short r is padded to 32 bytes" [ "$(der_hex 01 "$HIGH")" = "${ZEROS%??}01$HIGH" ]
check "an s with the top bit set (33 bytes in DER) is cut to 32" [ "$(der_hex 01 "$HIGH")" = "${ZEROS%??}01$HIGH" ]
check_not "an empty signature is not turned into zeros" der_signature_to_raw_hex < /dev/null
check "a short s is padded to 32 bytes" [ "$(der_hex "$HIGH" 02)" = "$HIGH${ZEROS%??}02" ]

# ---------------------------------------------------------------- fixtures

new_repository() { # name -> prints the repository root
  # The key and the stand-in API's files live beside the repository, not in it.
  local base="$WORK/$1" root="$WORK/$1/repo"
  mkdir -p "$root/apple/scripts" "$base/keys" "$base/fake"
  cp -R "$SCRIPTS/lib" "$root/apple/scripts/lib"
  cp "$SCRIPTS/register-bundle-id.sh" "$SCRIPTS/testflight.sh" "$root/apple/scripts/"
  : > "$root/apple/ExportOptions.plist"
  printf '// local values\nDEVELOPMENT_TEAM = ABCDE12345 // my team\nPRODUCT_BUNDLE_IDENTIFIER = com.acme.agency\n' > "$root/apple/Local.xcconfig"
  cp "$KEY" "$base/keys/AuthKey.p8"
  printf '%s' '{"data":[]}' > "$base/fake/bundle_ids.json"
  printf '%s' '{"data":[]}' > "$base/fake/capabilities.json"
  : > "$base/fake/log"
  printf '%s' "$root"
}

# run_script ROOT SCRIPT [ENV=value ...] -> output in $OUTPUT, status in $STATUS
run_script() {
  local root="$1" script="$2" base
  base=$(dirname "$root")
  shift 2
  OUTPUT=$(env -i HOME="$WORK/home" PATH="$HERE/fakes:$PATH" \
    FAKE_DIR="$base/fake" FAKE_LOG="$base/fake/log" DERIVED_DATA_ROOT="$root/derived" \
    ASC_API_ROOT=https://asc.invalid \
    "$@" "$root/apple/scripts/$script" 2>&1)
  STATUS=$?
}
VALID_ENV=(ASC_KEY_ID=ABCDE12345 ASC_ISSUER_ID=11111111-2222-3333-4444-555555555555)
requests() { grep -c '' "$1/../fake/log"; }

echo "credentials"
ROOT=$(new_repository credentials)
absolute_key_path() { # a relative key path, the directory to resolve it from
  (
    cd "$2" || exit 1
    ASC_KEY_PATH="$1" ASC_KEY_ID=ABCDE12345 ASC_ISSUER_ID=11111111-2222-3333-4444-555555555555
    load_asc_credentials "$ROOT"
    [ "${#PROBLEMS[@]}" -eq 0 ] && printf '%s' "$ASC_KEY_PATH"
  )
}
KEY_DIRECTORY=$(cd "$ROOT/../keys" && pwd -P)
check "a relative key path becomes absolute" [ "$(absolute_key_path AuthKey.p8 "$ROOT/../keys")" = "$KEY_DIRECTORY/AuthKey.p8" ]
# A literal "~/" is the case under test.
# shellcheck disable=SC2088
check "a key path with ~/ is expanded" [ "$(HOME="$KEY_DIRECTORY" absolute_key_path '~/AuthKey.p8' /)" = "$KEY_DIRECTORY/AuthKey.p8" ]

# ---------------------------------------------------------------- register-bundle-id.sh

echo "register-bundle-id.sh: checks before it calls anything"
ROOT=$(new_repository reg-checks)
run_script "$ROOT" register-bundle-id.sh
check "without any key setting it fails" [ "$STATUS" -ne 0 ]
check "it names the three missing settings" contains "$OUTPUT" "ASC_KEY_PATH is not set"
check "it names ASC_KEY_ID" contains "$OUTPUT" "ASC_KEY_ID is not set"
check "it names ASC_ISSUER_ID" contains "$OUTPUT" "ASC_ISSUER_ID is not set"
check "it sent no request" [ "$(requests "$ROOT")" -eq 0 ]

run_script "$ROOT" register-bundle-id.sh ASC_KEY_PATH="$WORK/missing.p8" ASC_KEY_ID=short ASC_ISSUER_ID=nope
check "a missing key file is reported" contains "$OUTPUT" "ASC_KEY_PATH does not name a readable file"
check "a malformed key ID is reported" contains "$OUTPUT" "ASC_KEY_ID is not a 10 character key ID"
check "a malformed issuer ID is reported" contains "$OUTPUT" "ASC_ISSUER_ID is not an issuer ID"

mkdir -p "$ROOT/apple/secrets"
cp "$KEY" "$ROOT/apple/secrets/AuthKey.p8"
run_script "$ROOT" register-bundle-id.sh ASC_KEY_PATH="$ROOT/apple/secrets/AuthKey.p8" "${VALID_ENV[@]}"
check "a key file inside the repository is refused" contains "$OUTPUT" "inside the repository"

rm "$ROOT/apple/Local.xcconfig"
run_script "$ROOT" register-bundle-id.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}"
check "a missing Local.xcconfig is reported" contains "$OUTPUT" "apple/Local.xcconfig is missing"

printf 'DEVELOPMENT_TEAM = XXXXXXXXXX\nPRODUCT_BUNDLE_IDENTIFIER = com.example.agency\n' > "$ROOT/apple/Local.xcconfig"
run_script "$ROOT" register-bundle-id.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}"
check "the placeholder team is refused" contains "$OUTPUT" "DEVELOPMENT_TEAM in apple/Local.xcconfig is not set"
check "the placeholder bundle ID is refused" contains "$OUTPUT" "still a placeholder"
check "it sent no request after those problems" [ "$(requests "$ROOT")" -eq 0 ]

echo "register-bundle-id.sh: against a stand-in API"
ROOT=$(new_repository reg-create)
run_script "$ROOT" register-bundle-id.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}"
LOG=$(cat "$ROOT/../fake/log")
check "it succeeds" [ "$STATUS" -eq 0 ]
check "it registers the bundle ID from Local.xcconfig for iOS" contains "$LOG" '"identifier":"com.acme.agency"'
check "it registers the widget extension's bundle ID, the app's with .widgets" contains "$LOG" '"identifier":"com.acme.agency.widgets"'
check "it names the platform" contains "$LOG" '"platform":"IOS"'
check "it enables Push Notifications on the new bundle ID" contains "$LOG" '"capabilityType":"PUSH_NOTIFICATIONS"'
check "it links the capability to the new bundle ID" contains "$LOG" '"id":"NEW-BUNDLE"'
check "it links a capability to the widget extension's bundle ID" contains "$LOG" '"id":"NEW-WIDGET"'
check "it says what it did" contains "$OUTPUT" "Registered bundle ID com.acme.agency."
check "it says it registered the widget extension" contains "$OUTPUT" "Registered bundle ID com.acme.agency.widgets."
check "it enables Associated Domains on the new bundle ID" contains "$LOG" '"capabilityType":"ASSOCIATED_DOMAINS"'
check "it enables App Groups" contains "$LOG" '"capabilityType":"APP_GROUPS"'
check "it enables App Groups on both bundle IDs" [ "$(grep -c '"capabilityType":"APP_GROUPS"' "$ROOT/../fake/log")" -eq 2 ]
check "it makes four capabilities and two bundle IDs" [ "$(grep -c '^POST' "$ROOT/../fake/log")" -eq 6 ]
check "it says it enabled Push Notifications" contains "$OUTPUT" "Enabled Push Notifications."
check "it says it enabled Associated Domains" contains "$OUTPUT" "Enabled Associated Domains."
check "it says the app group is not made by the API" contains "$OUTPUT" "group.com.acme.agency must exist"
AUTH_LINE=$(sed -n '1p' "$ROOT/../fake/log.auth")
BEARER=${AUTH_LINE#*Bearer }
BEARER=${BEARER%\"*}
check "the request carries a token that verifies" signature_matches "$BEARER" "$PUBLIC_KEY"
check "the token is not printed" [ -z "$(printf '%s' "$OUTPUT" | grep -F "$BEARER")" ]
check "the key file is not printed" [ -z "$(printf '%s' "$OUTPUT" | grep -F 'PRIVATE KEY')" ]
check "the token is not on a command line" [ -z "$(grep -F "$BEARER" "$ROOT/../fake/log")" ]

ALL_CAPABILITIES='{"data":[{"attributes":{"capabilityType":"PUSH_NOTIFICATIONS"}},{"attributes":{"capabilityType":"ASSOCIATED_DOMAINS"}},{"attributes":{"capabilityType":"APP_GROUPS"}}]}'
BOTH_BUNDLES='{"data":[{"id":"B1","attributes":{"identifier":"com.acme.agency"}},{"id":"B2","attributes":{"identifier":"com.acme.agency.widgets"}}]}'

ROOT=$(new_repository reg-existing)
printf '%s' "$BOTH_BUNDLES" > "$ROOT/../fake/bundle_ids.json"
printf '%s' "$ALL_CAPABILITIES" > "$ROOT/../fake/capabilities.json"
run_script "$ROOT" register-bundle-id.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}"
check "everything in place: it succeeds" [ "$STATUS" -eq 0 ]
check "everything in place: it only reads" [ -z "$(grep -E '^(POST|PUT|PATCH|DELETE)' "$ROOT/../fake/log")" ]
check "everything in place: it says so" contains "$OUTPUT" "is already registered"
check "everything in place: it says Push is on" contains "$OUTPUT" "Push Notifications is already enabled."
check "everything in place: it says Associated Domains is on" contains "$OUTPUT" "Associated Domains is already enabled."
check "everything in place: it says App Groups is on" contains "$OUTPUT" "App Groups is already enabled."

ROOT=$(new_repository reg-no-domains)
printf '%s' "$BOTH_BUNDLES" > "$ROOT/../fake/bundle_ids.json"
printf '%s' '{"data":[{"attributes":{"capabilityType":"PUSH_NOTIFICATIONS"}},{"attributes":{"capabilityType":"APP_GROUPS"}}]}' > "$ROOT/../fake/capabilities.json"
run_script "$ROOT" register-bundle-id.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}"
check "Push and App Groups without Associated Domains: it enables only Associated Domains" [ "$(grep -c '^POST' "$ROOT/../fake/log")" -eq 1 ]
check "Push and App Groups without Associated Domains: it is the missing one" contains "$(cat "$ROOT/../fake/log")" '"capabilityType":"ASSOCIATED_DOMAINS"'

ROOT=$(new_repository reg-no-groups)
printf '%s' "$BOTH_BUNDLES" > "$ROOT/../fake/bundle_ids.json"
printf '%s' '{"data":[{"attributes":{"capabilityType":"PUSH_NOTIFICATIONS"}},{"attributes":{"capabilityType":"ASSOCIATED_DOMAINS"}}]}' > "$ROOT/../fake/capabilities-B1.json"
printf '%s' '{"data":[]}' > "$ROOT/../fake/capabilities-B2.json"
run_script "$ROOT" register-bundle-id.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}"
check "no App Groups anywhere: it enables it on both" [ "$(grep -c '"capabilityType":"APP_GROUPS"' "$ROOT/../fake/log")" -eq 2 ]
check "no App Groups anywhere: it enables it on the app" contains "$(cat "$ROOT/../fake/log")" '"id":"B1"'
check "no App Groups anywhere: and on the widget extension" contains "$(cat "$ROOT/../fake/log")" '"id":"B2"'

ROOT=$(new_repository reg-no-push)
printf '%s' "$BOTH_BUNDLES" > "$ROOT/../fake/bundle_ids.json"
printf '%s' '{"data":[{"attributes":{"capabilityType":"ICLOUD"}}]}' > "$ROOT/../fake/capabilities-B1.json"
printf '%s' '{"data":[{"attributes":{"capabilityType":"APP_GROUPS"}}]}' > "$ROOT/../fake/capabilities-B2.json"
run_script "$ROOT" register-bundle-id.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}"
check "bundle ID without Push: it enables the app's three capabilities" [ "$(grep -c '^POST' "$ROOT/../fake/log")" -eq 3 ]
check "bundle ID without Push: it uses the existing bundle ID" contains "$(cat "$ROOT/../fake/log")" '"id":"B1"'

ROOT=$(new_repository reg-longer)
printf '%s' '{"data":[{"id":"B9","attributes":{"identifier":"com.acme.agency.widget"}}]}' > "$ROOT/../fake/bundle_ids.json"
run_script "$ROOT" register-bundle-id.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}"
check "a longer identifier that the filter also returns is not mistaken for ours" contains "$OUTPUT" "Registered bundle ID com.acme.agency."
check "nor is it mistaken for the widget extension's" contains "$OUTPUT" "Registered bundle ID com.acme.agency.widgets."

ROOT=$(new_repository reg-denied)
printf '403' > "$ROOT/../fake/status"
run_script "$ROOT" register-bundle-id.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}"
check "an API error stops the script" [ "$STATUS" -ne 0 ]
check "it reports the status" contains "$OUTPUT" "HTTP 403"
check "it reports Apple's reason" contains "$OUTPUT" "This key may not do that."
check "it stopped after the first request" [ "$(requests "$ROOT")" -eq 1 ]

# ---------------------------------------------------------------- testflight.sh

echo "testflight.sh: checks before it builds"
ROOT=$(new_repository tf-checks)
run_script "$ROOT" testflight.sh
check "without key settings it fails" [ "$STATUS" -ne 0 ]
check "it names the missing settings" contains "$OUTPUT" "ASC_KEY_PATH is not set"
check "it ran no tool" [ "$(requests "$ROOT")" -eq 0 ]
rm "$ROOT/apple/Local.xcconfig"
run_script "$ROOT" testflight.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}"
check "a missing Local.xcconfig stops it" contains "$OUTPUT" "apple/Local.xcconfig is missing"
check "it ran no tool without Local.xcconfig" [ "$(requests "$ROOT")" -eq 0 ]
ROOT=$(new_repository tf-number)
run_script "$ROOT" testflight.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}" BUILD_NUMBER=latest
check "a build number that is not a number is refused" contains "$OUTPUT" "BUILD_NUMBER must be numbers"

echo "testflight.sh: with stand-in tools"
ROOT=$(new_repository tf-run)
mkdir -p "$ROOT/derived/Other-abcdefghijklmnopqrstuvwxyzab" "$ROOT/derived/Agency-short"
run_script "$ROOT" testflight.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}" BUILD_NUMBER=202611251830
LOG=$(cat "$ROOT/../fake/log")
check "it succeeds" [ "$STATUS" -eq 0 ]
check "it generates the project first" [ "$(sed -n '1p' "$ROOT/../fake/log")" = "xcodegen generate --quiet" ]
REAL_KEY="$(cd "$ROOT/../keys" && pwd -P)/AuthKey.p8"
ARCHIVE=$(grep 'xcodebuild archive' "$ROOT/../fake/log")
EXPORT=$(grep 'xcodebuild -exportArchive' "$ROOT/../fake/log")
check "it archives the Release configuration for a device" contains "$ARCHIVE" "-configuration Release -destination generic/platform=iOS"
check "it sets the build number" contains "$ARCHIVE" "CURRENT_PROJECT_VERSION=202611251830"
check "the archive step has the key flags" contains "$ARCHIVE" "-allowProvisioningUpdates -authenticationKeyPath $REAL_KEY -authenticationKeyID ABCDE12345 -authenticationKeyIssuerID 11111111-2222-3333-4444-555555555555"
check "the export step has the key flags" contains "$EXPORT" "-allowProvisioningUpdates -authenticationKeyPath $REAL_KEY -authenticationKeyID ABCDE12345"
check "it exports with ExportOptions.plist" contains "$EXPORT" "-exportOptionsPlist ExportOptions.plist"
check "it archives before it exports" [ "$(grep -n 'xcodebuild' "$ROOT/../fake/log" | sed -n '1p' | grep -c archive)" -eq 1 ]
check "it deletes the build folder" [ ! -e "$ROOT/apple/build" ]
check "it deletes this project's DerivedData" [ ! -e "$ROOT/derived/Agency-abcdefghijklmnopqrstuvwxyzab" ]
check "it leaves another project's DerivedData" [ -d "$ROOT/derived/Other-abcdefghijklmnopqrstuvwxyzab" ]
check "it leaves a folder that only looks similar" [ -d "$ROOT/derived/Agency-short" ]
check "it prints no key setting" [ -z "$(printf '%s' "$OUTPUT" | grep -E 'AuthKey|ABCDE12345|11111111-2222')" ]
check "it says the build is uploaded" contains "$OUTPUT" "build 202611251830 is uploaded"
check "outside a git checkout the build says its source is unknown" contains "$ARCHIVE" "AGENCY_BUILD_COMMIT=unknown AGENCY_BUILD_DESCRIBE=unknown"
UPLOAD_RECORD=$(cat "$ROOT/apple/uploads.log")
check "it records the upload" contains "$UPLOAD_RECORD" "build 202611251830"$'\t'"version unknown"$'\t'"commit unknown"$'\t'"unknown"
check "the record is one line" [ "$(printf '%s\n' "$UPLOAD_RECORD" | wc -l | tr -d ' ')" -eq 1 ]

echo "testflight.sh: the source of a build"
ROOT=$(new_repository tf-source)
printf 'settings:\n  base:\n    MARKETING_VERSION: "0.4.2"\n' > "$ROOT/apple/project.yml"
GIT=(git -C "$ROOT" -c user.name=test -c user.email=test@example.com -c commit.gpgsign=false -c tag.gpgsign=false)
"${GIT[@]}" init -q -b main
"${GIT[@]}" add -A
"${GIT[@]}" commit -q -m "feat: first"
"${GIT[@]}" tag apple-v0.4.2
echo more >> "$ROOT/apple/project.yml"
printf 'settings:\n  base:\n    MARKETING_VERSION: "0.4.2"\n' > "$ROOT/apple/project.yml"
"${GIT[@]}" commit -q --allow-empty -m "fix: second"
"${GIT[@]}" tag v9.9.9
COMMIT=$("${GIT[@]}" rev-parse --short=12 HEAD)
run_script "$ROOT" testflight.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}" BUILD_NUMBER=202611260900
ARCHIVE=$(grep 'xcodebuild archive' "$ROOT/../fake/log")
check "the build carries the commit it was made from" contains "$ARCHIVE" "AGENCY_BUILD_COMMIT=$COMMIT"
check "and how far it is from the last apple tag (a tag of another artifact is not one)" contains "$ARCHIVE" "AGENCY_BUILD_DESCRIBE=apple-v0.4.2-1-g$COMMIT "
check "it records the commit, the describe, and the version" \
  contains "$(cat "$ROOT/apple/uploads.log")" "build 202611260900"$'\t'"version 0.4.2"$'\t'"commit $COMMIT"$'\t'"apple-v0.4.2-1-g$COMMIT"
check "a clean tree gives no warning" [ -z "$(printf '%s' "$OUTPUT" | grep -F 'uncommitted changes')" ]
echo changed >> "$ROOT/apple/project.yml"
run_script "$ROOT" testflight.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}" BUILD_NUMBER=202611260901
check "a working tree with changes is built, and says so" contains "$OUTPUT" "uncommitted changes; this build is recorded as apple-v0.4.2-1-g$COMMIT-dirty"
check "and the record says -dirty" contains "$(tail -n 1 "$ROOT/apple/uploads.log")" "apple-v0.4.2-1-g$COMMIT-dirty"
ROOT=$(new_repository tf-failed-upload)
run_script "$ROOT" testflight.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}" FAKE_FAIL=export
check "an upload that failed is not recorded" [ ! -e "$ROOT/apple/uploads.log" ]

ROOT=$(new_repository tf-default-number)
run_script "$ROOT" testflight.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}"
check "without BUILD_NUMBER it uses the UTC time, to the minute" \
  grep -Eq 'CURRENT_PROJECT_VERSION=[0-9]{12} ' "$ROOT/../fake/log"

echo "testflight.sh: when a step fails"
ROOT=$(new_repository tf-archive-fails)
run_script "$ROOT" testflight.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}" FAKE_FAIL=archive
check "a failed archive fails the script" [ "$STATUS" -ne 0 ]
check "a failed archive says so" contains "$OUTPUT" "the archive failed"
check "a failed archive is not exported" [ -z "$(grep -F exportArchive "$ROOT/../fake/log")" ]
check "a failed archive still deletes build/" [ ! -e "$ROOT/apple/build" ]
check "a failed archive still deletes DerivedData" [ ! -e "$ROOT/derived/Agency-abcdefghijklmnopqrstuvwxyzab" ]
ROOT=$(new_repository tf-export-fails)
run_script "$ROOT" testflight.sh ASC_KEY_PATH="$ROOT/../keys/AuthKey.p8" "${VALID_ENV[@]}" FAKE_FAIL=export
check "a failed upload fails the script" [ "$STATUS" -ne 0 ]
check "a failed upload says so" contains "$OUTPUT" "the export or the upload failed"
check "a failed upload does not claim success" [ -z "$(printf '%s' "$OUTPUT" | grep -F 'is uploaded')" ]
check "a failed upload still deletes build/" [ ! -e "$ROOT/apple/build" ]

echo
if [ "$FAILURES" -eq 0 ]; then
  echo "All checks passed."
else
  echo "$FAILURES check(s) failed."
  exit 1
fi
