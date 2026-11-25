import vectors from '../../../testdata/vectors.json' with { type: 'json' }
import type { DeviceList, DeviceRequest } from '../api/clientApi'
import { fromBase64 } from './bytes'
import { addChangeFor } from './changes'
import { canonicalJson, type Json } from './canonical'
import { deviceKeyOf } from './deviceKey'
import { buildEnvelope, newEnvelopeId, signEnvelope } from './envelope'
import { deviceIdIn, enrollmentOf, orgsLacking, requestsFromOthers } from './enrollment'
import { deviceIdOf } from './fingerprint'
import { signedText } from './signing'
import { timestampOf } from './time'

const PKCS8_PREFIX = Uint8Array.from([0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20])
const hexBytes = (hex: string): Uint8Array<ArrayBuffer> => Uint8Array.from(hex.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16))

async function vectorDeviceKey() {
  const privateKey = await crypto.subtle.importKey('pkcs8', Uint8Array.from([...PKCS8_PREFIX, ...hexBytes(vectors.device.seed)]), 'Ed25519', false, ['sign'])
  const publicKey = await crypto.subtle.importKey('raw', fromBase64(vectors.device.publicKey), 'Ed25519', true, ['verify'])
  return deviceKeyOf({ privateKey, publicKey })
}

const AT = new Date('2026-11-24T12:00:00Z')

describe('a decision as an envelope', () => {
  const answer = { org: 'acme', team: 'app', answerWith: { kind: 'answer', target: 'r-20260101-000000-abcd' }, title: 'A request', choice: 'ア', text: '' } as const
  const envelope = buildEnvelope({ answer, account: 'alice', id: '00112233445566778899aabbccddeeff', at: AT })

  it('is the vector envelope, byte for byte, and keeps the notes only the log holds out of it', () => {
    expect(canonicalJson(envelope as unknown as Json)).toBe(vectors.envelope.canonical)
    expect(JSON.stringify(envelope)).not.toContain('A request')
  })

  it('is signed with the vector’s signature by the device key, under the device’s id', async () => {
    const signed = await signEnvelope(await vectorDeviceKey(), 'laptop', envelope)
    expect(signed).toEqual({ body: envelope, signer: 'laptop', signature: vectors.envelope.signature })
  })

  it('leaves the choice out when there is none, and writes the time to the second', () => {
    const open = buildEnvelope({ answer: { ...answer, choice: undefined as never, text: 'Go.' }, account: 'alice', id: 'a'.repeat(32), at: new Date('2026-11-24T12:00:00.789Z') })
    expect(open.payload).toEqual({ text: 'Go.' })
    expect(open.at).toBe('2026-11-24T12:00:00Z')
  })

  it('gets a new id of 32 lowercase hex characters each time', () => {
    const ids = new Set(Array.from({ length: 20 }, newEnvelopeId))
    expect(ids.size).toBe(20)
    for (const id of ids) expect(id).toMatch(/^[0-9a-f]{32}$/)
  })
})

describe('adding a device', () => {
  const request: DeviceRequest = { label: 'Laptop', publicKey: vectors.device.publicKey, deviceId: 'laptop', pairingCode: vectors.device.pairingCode, requestedAt: '2026-11-24T11:59:00Z' }

  it('is the vector device change, byte for byte', () => {
    expect(canonicalJson(addChangeFor(request, 'acme', 'alice', AT) as unknown as Json)).toBe(vectors.deviceChange.canonical)
  })

  it('names a device by its pairing code', () => {
    expect(deviceIdOf(vectors.device.pairingCode)).toBe('d-dbc298251c51')
  })

  it('is signed as kind device, so it does not pass as a decision', async () => {
    const key = await vectorDeviceKey()
    const body = addChangeFor(request, 'acme', 'alice', AT) as unknown as Json
    const publicKey = await crypto.subtle.importKey('raw', fromBase64(vectors.device.publicKey), 'Ed25519', true, ['verify'])
    const signature = fromBase64(await key.sign(signedText('device', body)))
    expect(await crypto.subtle.verify('Ed25519', publicKey, signature, signedText('device', body))).toBe(true)
    expect(await crypto.subtle.verify('Ed25519', publicKey, signature, signedText('envelope', body))).toBe(false)
  })
})

describe('where this device stands', () => {
  const MINE = vectors.device.publicKey
  const OTHER = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA='
  const device = (id: string, publicKey: string) => ({ id, label: id, publicKey, addedAt: '2026-11-24T12:00:00Z' })
  const list = (orgs: DeviceList['orgs'], requests: DeviceRequest[] = []): DeviceList => ({ orgs, requests })
  const asked: DeviceRequest = { label: 'Phone', publicKey: OTHER, deviceId: 'd-000000000000', pairingCode: '0000-0000-0000', requestedAt: '2026-11-24T12:00:00Z' }

  it('is enrolled once any org lists its key, under the id that org gave it', () => {
    expect(enrollmentOf(MINE, list([{ org: 'acme', devices: [device('d-1', OTHER)] }, { org: 'lab', devices: [device('d-2', MINE)] }]))).toEqual({ status: 'enrolled', deviceId: 'd-2' })
  })

  it('is pending while its request waits, and none when it has not asked', () => {
    expect(enrollmentOf(OTHER, list([{ org: 'acme', devices: [] }], [asked]))).toEqual({ status: 'pending', request: asked })
    expect(enrollmentOf(MINE, list([{ org: 'acme', devices: [] }], [asked]))).toEqual({ status: 'none' })
  })

  it('signs only for an org that holds its key', () => {
    const state = list([{ org: 'acme', devices: [device('d-1', MINE)] }, { org: 'lab', devices: [] }])
    expect(deviceIdIn(state, MINE, 'acme')).toBe('d-1')
    expect(deviceIdIn(state, MINE, 'lab')).toBeUndefined()
    expect(deviceIdIn(state, MINE, 'nowhere')).toBeUndefined()
  })

  it('adds a new device to every org that lacks it, and offers only requests from others', () => {
    const state = list([{ org: 'acme', devices: [device('d-1', MINE)] }, { org: 'lab', devices: [device('d-1', MINE)] }, { org: 'new', devices: [] }], [asked])
    expect(orgsLacking(state, OTHER)).toEqual(['acme', 'lab', 'new'])
    expect(orgsLacking(state, MINE)).toEqual(['new'])
    expect(requestsFromOthers(state, MINE)).toEqual([asked])
    expect(requestsFromOthers(state, OTHER)).toEqual([])
  })
})

it('writes times to the second', () => {
  expect(timestampOf(new Date('2026-11-24T12:00:00.999Z'))).toBe('2026-11-24T12:00:00Z')
})
