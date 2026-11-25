import { fromBase64, toHex } from './bytes'

const PUBLIC_KEY_BYTES = 32

/** The 32 bytes of an Ed25519 public key written as base64, or an error when it is not one. */
export function publicKeyBytes(publicKey: string): Uint8Array<ArrayBuffer> {
  const bytes = fromBase64(publicKey.trim())
  if (bytes.length !== PUBLIC_KEY_BYTES) throw new RangeError('A public key is 32 bytes.')
  return bytes
}

/** The first 16 bytes of the key's SHA-256, as hex (32 characters). */
export async function fingerprintOf(publicKey: string): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', publicKeyBytes(publicKey))
  return toHex(new Uint8Array(digest).slice(0, 16))
}

/**
 * The code a person compares on two devices before one adds the other: the fingerprint's first 12 hex
 * characters in groups of 4 (`dbc2-9825-1c51`). 48 bits, so a node relaying the key cannot find another
 * key with the same code while the person is pairing.
 */
export async function pairingCodeOf(publicKey: string): Promise<string> {
  const fingerprint = await fingerprintOf(publicKey)
  return [fingerprint.slice(0, 4), fingerprint.slice(4, 8), fingerprint.slice(8, 12)].join('-')
}
