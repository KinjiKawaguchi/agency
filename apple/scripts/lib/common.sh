# shellcheck shell=bash
# Shared by the scripts in apple/scripts. Sourced, never run on its own.
# Works with the bash 3.2 that macOS ships.

# Stops at once, with a message on stderr.
die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

# Problems are collected so that one run reports everything that is missing.
PROBLEMS=()

problem() {
  PROBLEMS+=("$*")
}

exit_if_problems() {
  if [ "${#PROBLEMS[@]}" -eq 0 ]; then
    return 0
  fi
  printf 'Cannot continue:\n' >&2
  local entry
  for entry in "${PROBLEMS[@]}"; do
    printf '  - %s\n' "$entry" >&2
  done
  exit 1
}

# Prints the value of KEY in an .xcconfig file: the text after "=", without a trailing comment.
# Prints nothing when the file does not set it.
xcconfig_value() {
  awk -v key="$2" '
    /^[[:space:]]*\/\// { next }
    {
      name = $0
      sub(/=.*/, "", name)
      gsub(/[[:space:]]/, "", name)
    }
    name == key && index($0, "=") {
      value = $0
      sub(/^[^=]*=/, "", value)
      sub(/[[:space:]]*\/\/.*$/, "", value)
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
      print value
      exit
    }
  ' "$1"
}

# Reads the signing values from apple/Local.xcconfig into DEVELOPMENT_TEAM and BUNDLE_ID.
# $1: the apple/ directory. Reports through problem(); the caller runs exit_if_problems.
load_local_config() {
  local file="$1/Local.xcconfig"
  DEVELOPMENT_TEAM=""
  BUNDLE_ID=""
  if [ ! -f "$file" ]; then
    problem "apple/Local.xcconfig is missing: copy apple/Local.xcconfig.example to it and fill in your own values"
    return 0
  fi
  DEVELOPMENT_TEAM=$(xcconfig_value "$file" DEVELOPMENT_TEAM)
  BUNDLE_ID=$(xcconfig_value "$file" PRODUCT_BUNDLE_IDENTIFIER)

  if [ -z "$DEVELOPMENT_TEAM" ] || [ "$DEVELOPMENT_TEAM" = "XXXXXXXXXX" ]; then
    problem "DEVELOPMENT_TEAM in apple/Local.xcconfig is not set to your team ID"
  elif ! printf '%s' "$DEVELOPMENT_TEAM" | grep -Eq '^[A-Z0-9]{10}$'; then
    problem "DEVELOPMENT_TEAM in apple/Local.xcconfig is not a 10 character team ID"
  fi

  if [ -z "$BUNDLE_ID" ]; then
    problem "PRODUCT_BUNDLE_IDENTIFIER in apple/Local.xcconfig is not set"
  elif ! printf '%s' "$BUNDLE_ID" | grep -Eq '^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$'; then
    problem "PRODUCT_BUNDLE_IDENTIFIER in apple/Local.xcconfig is not a bundle ID (reverse domain name)"
  elif printf '%s' "$BUNDLE_ID" | grep -Eq '^(com|org)\.example\.'; then
    problem "PRODUCT_BUNDLE_IDENTIFIER in apple/Local.xcconfig is still a placeholder"
  fi
}

# Checks ASC_KEY_PATH, ASC_KEY_ID and ASC_ISSUER_ID, the App Store Connect API key settings. On
# success ASC_KEY_PATH is the key file as an absolute path. $1: the repository root.
# The values are never printed.
load_asc_credentials() {
  local name
  for name in ASC_KEY_PATH ASC_KEY_ID ASC_ISSUER_ID; do
    if [ -z "${!name:-}" ]; then
      problem "$name is not set (see docs/apple.md, \"Release in two commands\")"
    fi
  done

  if [ -n "${ASC_KEY_PATH:-}" ]; then
    # A quoted assignment leaves a literal "~/" in the variable, which is expanded here.
    # shellcheck disable=SC2088
    case "$ASC_KEY_PATH" in
      "~/"*) ASC_KEY_PATH="$HOME/${ASC_KEY_PATH#\~/}" ;;
    esac
    if [ ! -f "$ASC_KEY_PATH" ] || [ ! -r "$ASC_KEY_PATH" ]; then
      problem "ASC_KEY_PATH does not name a readable file"
    elif key_is_inside "$ASC_KEY_PATH" "$1"; then
      problem "the API key file is inside the repository: keep it outside, and never commit it"
    else
      # The scripts change directory, and xcodebuild wants an absolute path.
      ASC_KEY_PATH="$(cd "$(dirname "$ASC_KEY_PATH")" && pwd -P)/$(basename "$ASC_KEY_PATH")"
    fi
  fi
  if [ -n "${ASC_KEY_ID:-}" ] && ! printf '%s' "$ASC_KEY_ID" | grep -Eq '^[A-Z0-9]{10}$'; then
    problem "ASC_KEY_ID is not a 10 character key ID"
  fi
  if [ -n "${ASC_ISSUER_ID:-}" ] &&
    ! printf '%s' "$ASC_ISSUER_ID" | grep -Eq '^[0-9A-Fa-f]{8}(-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}$'; then
    problem "ASC_ISSUER_ID is not an issuer ID (a UUID)"
  fi
}

# Whether a file lies under a directory, after resolving links.
key_is_inside() {
  local directory file_directory
  file_directory=$(cd "$(dirname "$1")" && pwd -P)
  directory=$(cd "$2" && pwd -P)
  case "$file_directory/" in
    "$directory/"*) return 0 ;;
    *) return 1 ;;
  esac
}

require_command() {
  command -v "$1" > /dev/null 2>&1 || problem "$1 is not installed${2:+ ($2)}"
}
