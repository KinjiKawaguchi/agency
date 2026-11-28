#!/usr/bin/env bash
# Makes the test-only keys and certificates of the OpenID Connect integration tests (tests/common/oidc_issuer.rs):
#   oidc-test-ca.der          a test CA, which the node's key fetch is told to trust in the tests
#   oidc-test-leaf.der        the certificate of the loopback issuer ("localhost" and 127.0.0.1), signed by it
#   oidc-test-leaf-key.der    its private key (PKCS#8)
#   oidc-rotated-test-key.der a second RSA key (PKCS#1), for the key rotation tests
# Test only, never reused: the private keys here are published with the repository, and nothing real trusts
# them (the CA is added to a client only by `HttpCerts::trusting`, which the tests alone call). Do not copy them
# for anything else. None of them is used anywhere but in these tests. They last 100 years, so nothing needs making again; run this
# only to replace them, and then put the new modulus of the rotated key in `ROTATED_KEY_N`:
#   openssl rsa -in oidc-rotated-test-key.der -inform DER -modulus -noout | sed 's/Modulus=//' | xxd -r -p | base64 -w0 | tr '+/' '-_' | tr -d '='
set -euo pipefail
cd "$(dirname "$0")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cat > "$work/ca.cnf" <<'CONFIG'
[req]
distinguished_name = dn
prompt = no
x509_extensions = ext
[dn]
CN = Agency test issuer CA (test only)
[ext]
basicConstraints = critical,CA:TRUE,pathlen:0
keyUsage = critical,keyCertSign
subjectKeyIdentifier = hash
CONFIG
cat > "$work/leaf.cnf" <<'CONFIG'
[req]
distinguished_name = dn
prompt = no
[dn]
CN = localhost
[ext]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature
extendedKeyUsage = serverAuth
subjectAltName = DNS:localhost,IP:127.0.0.1
CONFIG

openssl ecparam -name prime256v1 -genkey -noout -out "$work/ca.key"
openssl req -x509 -new -key "$work/ca.key" -sha256 -days 36500 -config "$work/ca.cnf" -out "$work/ca.pem"
openssl ecparam -name prime256v1 -genkey -noout -out "$work/leaf-ec.key"
openssl pkcs8 -topk8 -nocrypt -in "$work/leaf-ec.key" -outform DER -out oidc-test-leaf-key.der
openssl req -new -key "$work/leaf-ec.key" -config "$work/leaf.cnf" -out "$work/leaf.csr"
openssl x509 -req -in "$work/leaf.csr" -CA "$work/ca.pem" -CAkey "$work/ca.key" -CAcreateserial -sha256 -days 36500 \
  -extfile "$work/leaf.cnf" -extensions ext -out "$work/leaf.pem"
openssl x509 -in "$work/ca.pem" -outform DER -out oidc-test-ca.der
openssl x509 -in "$work/leaf.pem" -outform DER -out oidc-test-leaf.der
openssl genrsa -traditional 2048 | openssl rsa -traditional -outform DER -out oidc-rotated-test-key.der
echo "Wrote the OpenID Connect test keys and certificates in $PWD"
