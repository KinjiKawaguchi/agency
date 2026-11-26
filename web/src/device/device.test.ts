import { toBase64, fromBase64 } from './bytes'
import vectors from '../../../testdata/vectors.json' with { type: 'json' }
import { canonicalJson, type Json } from './canonical'
import { deviceKeyOf, generateKeyPair, loadOrCreateDeviceKey, type KeyStore } from './deviceKey'
import { fingerprintOf, pairingCodeOf, publicKeyBytes } from './fingerprint'
import { signDocument, signedText } from './signing'

// PKCS#8 for an Ed25519 key is this prefix followed by the 32-byte seed (RFC 8410).
const PKCS8_PREFIX = Uint8Array.from([0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20])

const hexBytes = (hex: string): Uint8Array<ArrayBuffer> => Uint8Array.from(hex.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16))

/** The vector's device key imported from its seed, like the device key a browser would hold, but not extractable. */
async function vectorKeyPair(): Promise<CryptoKeyPair> {
  const privateKey = await crypto.subtle.importKey('pkcs8', Uint8Array.from([...PKCS8_PREFIX, ...hexBytes(vectors.device.seed)]), 'Ed25519', false, ['sign'])
  const publicKey = await crypto.subtle.importKey('raw', fromBase64(vectors.device.publicKey), 'Ed25519', true, ['verify'])
  return { privateKey, publicKey }
}

const parse = (canonical: string): Json => JSON.parse(canonical) as Json

describe('canonical JSON', () => {
  it.each([
    ['org', vectors.org.canonical],
    ['org with a team plan', vectors.orgWithPlan.canonical],
    ['device change', vectors.deviceChange.canonical],
    ['P-256 device add', vectors.deviceChangeP256.add.canonical],
    ['P-256 device remove', vectors.deviceChangeP256.remove.canonical],
    ['envelope', vectors.envelope.canonical],
    ['comment envelope', vectors.commentEnvelope.canonical],
  ])('reproduces the bytes of the %s vector from its parsed body', (_name, canonical) => {
    expect(canonicalJson(parse(canonical))).toBe(canonical)
  })

  it.each(vectors.canonical.map((one) => [one.name, one.value, one.canonical] as const))(
    'writes the %s vector byte for byte',
    (_name, value, canonical) => {
      expect(canonicalJson(value as Json)).toBe(canonical)
    },
  )

  it('does not depend on the order the fields were written in', () => {
    expect(canonicalJson({ b: 1, a: { d: [3, 1], c: null }, 'z': 'z' })).toBe('{"a":{"d":[3,1]},"b":1,"z":"z"}')
    expect(canonicalJson({ z: 'z', a: 1 })).toBe(canonicalJson({ a: 1, z: 'z' }))
  })

  it('sorts keys by their UTF-8 bytes, not by UTF-16 code units', () => {
    // U+FF5E is one UTF-16 unit above any surrogate, but its UTF-8 bytes sort below U+1F600's.
    expect(canonicalJson({ '😀': 1, '～': 2 })).toBe('{"～":2,"😀":1}')
  })

  it('leaves out object fields that are null or undefined, and keeps null in arrays', () => {
    expect(canonicalJson({ a: null, b: undefined, c: [null, 1] })).toBe('{"c":[null,1]}')
  })

  it('escapes strings as JSON does and leaves other characters as they are', () => {
    expect(canonicalJson({ t: 'a"b\\c\n\u0001/ア' })).toBe('{"t":"a\\"b\\\\c\\n\\u0001/ア"}')
  })

  it('refuses a number that is not an integer, which the protocol cannot sign', () => {
    expect(() => canonicalJson({ a: 1.5 })).toThrow(RangeError)
    expect(() => canonicalJson({ a: Number.NaN })).toThrow(RangeError)
    expect(() => canonicalJson({ a: 2 ** 60 })).toThrow(RangeError)
  })
})

describe('keys', () => {
  it('gives the fingerprint and pairing code of the vectors', async () => {
    expect(await pairingCodeOf(vectors.device.publicKey)).toBe(vectors.device.pairingCode)
    expect(await fingerprintOf(vectors.publicKey)).toBe(vectors.fingerprint)
  })

  it('refuses something that is not a public key', async () => {
    await expect(pairingCodeOf('AAAA')).rejects.toThrow('32 bytes')
    expect(() => publicKeyBytes('not base64!')).toThrow('base64')
  })
})

describe('the device key', () => {
  it('signs the vector envelope with the bytes the vector holds', async () => {
    const key = await deviceKeyOf(await vectorKeyPair())
    expect(key.publicKey).toBe(vectors.device.publicKey)
    expect(key.pairingCode).toBe(vectors.device.pairingCode)
    const signed = await signDocument(key, 'envelope', parse(vectors.envelope.canonical), vectors.envelope.signer)
    expect(signed.signature).toBe(vectors.envelope.signature)
    expect(signed.signer).toBe('laptop')
    const comment = await signDocument(key, 'envelope', parse(vectors.commentEnvelope.canonical), vectors.commentEnvelope.signer)
    expect(comment.signature).toBe(vectors.commentEnvelope.signature)
  })

  it('signs under the kind line, so a signature for one kind is not valid for another', async () => {
    const pair = await vectorKeyPair()
    const body = parse(vectors.envelope.canonical)
    const signature = fromBase64(vectors.envelope.signature)
    expect(await crypto.subtle.verify('Ed25519', pair.publicKey, signature, signedText('envelope', body))).toBe(true)
    expect(await crypto.subtle.verify('Ed25519', pair.publicKey, signature, signedText('device', body))).toBe(false)
  })

  it('verifies the P-256 device vector with WebCrypto, which uses the same raw r and s', async () => {
    const vector = vectors.p256Device
    const point = fromBase64(vector.publicKey.replace(/^p256:/, ''))
    const key = await crypto.subtle.importKey('raw', point, { name: 'ECDSA', namedCurve: 'P-256' }, false, ['verify'])
    const text = signedText('envelope', parse(vectors.commentEnvelope.canonical))
    const ecdsa = { name: 'ECDSA', hash: 'SHA-256' }
    expect(await crypto.subtle.verify(ecdsa, key, fromBase64(vector.commentSignature), text)).toBe(true)
    expect(await crypto.subtle.verify(ecdsa, key, fromBase64(vector.commentSignature), signedText('device', parse(vectors.commentEnvelope.canonical)))).toBe(false)
  })

  it.each([
    ['add', vectors.deviceChangeP256.add],
    ['remove', vectors.deviceChangeP256.remove],
  ])('verifies the P-256 device change vector (%s) over its signed text', async (_name, vector) => {
    const point = fromBase64(vectors.deviceChangeP256.signerKey.replace(/^p256:/, ''))
    const key = await crypto.subtle.importKey('raw', point, { name: 'ECDSA', namedCurve: 'P-256' }, false, ['verify'])
    const ecdsa = { name: 'ECDSA', hash: 'SHA-256' }
    const text = signedText('device', parse(vector.canonical))
    expect(new TextDecoder().decode(text)).toBe(vector.signedText)
    expect(vector.signer).toBe(vectors.p256Device.deviceId)
    expect(await crypto.subtle.verify(ecdsa, key, fromBase64(vector.signature), text)).toBe(true)
    expect(await crypto.subtle.verify(ecdsa, key, fromBase64(vector.signature), signedText('envelope', parse(vector.canonical)))).toBe(false)
  })

  it('cannot be read out of the browser', async () => {
    const pair = await vectorKeyPair()
    await expect(crypto.subtle.exportKey('pkcs8', pair.privateKey)).rejects.toThrow()
    const generated = await generateKeyPair()
    expect(generated.privateKey.extractable).toBe(false)
    await expect(crypto.subtle.exportKey('pkcs8', generated.privateKey)).rejects.toThrow()
  })

  it('checks out against the public key it reports, for a freshly made key', async () => {
    const key = await deviceKeyOf(await generateKeyPair())
    const message = signedText('device', { op: 'add' })
    const publicKey = await crypto.subtle.importKey('raw', publicKeyBytes(key.publicKey), 'Ed25519', true, ['verify'])
    expect(await crypto.subtle.verify('Ed25519', publicKey, fromBase64(await key.sign(message)), message)).toBe(true)
    expect(fromBase64(key.publicKey)).toHaveLength(32)
  })
})

describe('the account key vectors, which use the other kinds', () => {
  const accountSigner = async () => {
    const privateKey = await crypto.subtle.importKey('pkcs8', Uint8Array.from([...PKCS8_PREFIX, ...hexBytes(vectors.seed)]), 'Ed25519', false, ['sign'])
    return { sign: async (message: Uint8Array<ArrayBuffer>) => toBase64(new Uint8Array(await crypto.subtle.sign('Ed25519', privateKey, message))) }
  }

  it.each([
    ['device', vectors.deviceChange],
    ['org', vectors.org],
  ] as const)('signs the %s vector with the bytes it holds', async (kind, vector) => {
    const signed = await signDocument(await accountSigner(), kind, parse(vector.canonical), 'alice')
    expect(signed.signature).toBe(vector.signature)
  })
})

describe('loadOrCreateDeviceKey', () => {
  const memoryStore = (): KeyStore & { saved: () => number } => {
    let kept: CryptoKeyPair | undefined
    let saves = 0
    return {
      load: () => Promise.resolve(kept),
      save: (pair) => {
        kept = pair
        saves += 1
        return Promise.resolve()
      },
      saved: () => saves,
    }
  }

  it('makes and stores a key the first time and gives the same one after', async () => {
    const store = memoryStore()
    const first = await loadOrCreateDeviceKey(store)
    const second = await loadOrCreateDeviceKey(store)
    expect(second.publicKey).toBe(first.publicKey)
    expect(store.saved()).toBe(1)
  })

  it('is unaffected by base64 helpers: round trip', () => {
    const bytes = Uint8Array.from({ length: 64 }, (_, index) => index * 4)
    expect(fromBase64(toBase64(bytes))).toEqual(bytes)
  })
})
