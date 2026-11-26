import { toBase64 } from './bytes'
import { pairingCodeOf } from './fingerprint'
import type { Signer } from './signing'

/**
 * This device's signing key. The private half is a non-extractable WebCrypto key: the page can ask it to
 * sign, but nothing, including this page's own code, can read it out.
 */
export interface DeviceKey extends Signer {
  /** The 32 bytes of the public key in base64, as org definitions and device changes carry it. */
  publicKey: string
  /** The code to compare with the one the other device shows (`dbc2-9825-1c51`). */
  pairingCode: string
}

export async function deviceKeyOf(pair: CryptoKeyPair): Promise<DeviceKey> {
  const publicKey = toBase64(new Uint8Array(await crypto.subtle.exportKey('raw', pair.publicKey)))
  return {
    publicKey,
    pairingCode: await pairingCodeOf(publicKey),
    sign: async (message) => toBase64(new Uint8Array(await crypto.subtle.sign('Ed25519', pair.privateKey, message))),
  }
}

/** Makes a key pair whose private half cannot be exported. */
export const generateKeyPair = (): Promise<CryptoKeyPair> => crypto.subtle.generateKey('Ed25519', false, ['sign', 'verify']) as Promise<CryptoKeyPair>

/** Where the key pair is kept between visits. A CryptoKey is stored as it is, never as bytes. */
export interface KeyStore {
  load: () => Promise<CryptoKeyPair | undefined>
  save: (pair: CryptoKeyPair) => Promise<void>
}

/** The key this device already has, or a new one that is stored before it is used; and whether it had to be made. */
export async function loadDeviceKey(store: KeyStore): Promise<{ key: DeviceKey; isCreated: boolean }> {
  const existing = await store.load()
  if (existing !== undefined) return { key: await deviceKeyOf(existing), isCreated: false }
  const created = await generateKeyPair()
  await store.save(created)
  return { key: await deviceKeyOf(created), isCreated: true }
}

export const loadOrCreateDeviceKey = async (store: KeyStore): Promise<DeviceKey> => (await loadDeviceKey(store)).key

/** What this page has of what a device key needs, so a wrapper app can see which part is missing. */
export function deviceKeyCapabilities(): { secureContext: boolean; webCrypto: boolean; indexedDb: boolean } {
  return { secureContext: window.isSecureContext, webCrypto: typeof crypto !== 'undefined' && typeof crypto.subtle !== 'undefined', indexedDb: typeof indexedDB !== 'undefined' }
}

/** Whether this browser can make Ed25519 keys (it needs a secure context and a recent browser). */
export async function canMakeDeviceKeys(): Promise<boolean> {
  try {
    await generateKeyPair()
    return true
  } catch {
    return false
  }
}
