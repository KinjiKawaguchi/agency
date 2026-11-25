# shellcheck shell=bash
# Builds the token the App Store Connect API asks for. Sourced, never run on its own.
# Needs openssl and perl. Requires lib/common.sh.

base64url() {
  openssl base64 -A | tr '+/' '-_' | tr -d '='
}

# openssl signs ECDSA as a DER sequence of two integers; a JWT carries the two numbers as one
# 64 byte string, each left-padded to 32 bytes. Reads the DER on stdin, prints 128 hex digits.
der_signature_to_raw_hex() {
  local integers r s
  integers=$(openssl asn1parse -inform DER | awk -F: '/INTEGER/ { print $NF }')
  r=$(printf '%s\n' "$integers" | sed -n '1p' | sed 's/^0*//')
  s=$(printf '%s\n' "$integers" | sed -n '2p' | sed 's/^0*//')
  if [ -z "$r" ] || [ -z "$s" ]; then
    return 1
  fi
  r=$(printf '%64s' "$r" | tr ' ' 0)
  s=$(printf '%64s' "$s" | tr ' ' 0)
  if [ "${#r}" -ne 64 ] || [ "${#s}" -ne 64 ]; then
    return 1
  fi
  printf '%s%s' "$r" "$s"
}

hex_to_base64url() {
  perl -e 'print pack("H*", $ARGV[0])' "$(cat)" | base64url
}

# asc_jwt KEY_FILE KEY_ID ISSUER_ID [NOW]
# Prints an ES256 JWT that is valid for ten minutes (Apple allows twenty). NOW is for tests.
asc_jwt() {
  local key_file="$1" key_id="$2" issuer_id="$3" now="${4:-$(date +%s)}"
  local header payload signing_input signature
  header=$(printf '{"alg":"ES256","kid":"%s","typ":"JWT"}' "$key_id")
  payload=$(printf '{"iss":"%s","iat":%s,"exp":%s,"aud":"appstoreconnect-v1"}' \
    "$issuer_id" "$now" "$((now + 600))")
  signing_input="$(printf '%s' "$header" | base64url).$(printf '%s' "$payload" | base64url)"
  signature=$(printf '%s' "$signing_input" |
    openssl dgst -sha256 -sign "$key_file" |
    der_signature_to_raw_hex |
    hex_to_base64url) || return 1
  [ -n "$signature" ] || return 1
  printf '%s.%s' "$signing_input" "$signature"
}
