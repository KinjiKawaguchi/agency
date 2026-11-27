// The devices part of the mock node: the Client API routes of docs/protocol.md that list devices, take a
// request to add one, take a signed change, and check the signature of a decision. It verifies signatures
// as the real node does, so the client's signing is exercised end to end.
import type { Device, DeviceChange, DeviceList, DeviceRequest, Envelope, Signed } from '../api/clientApi.ts'
import { fromBase64 } from '../device/bytes.ts'
import type { Json } from '../device/canonical.ts'
import { deviceIdOf, pairingCodeOf, publicKeyBytes } from '../device/fingerprint.ts'
import { signedText, type SignedKind } from '../device/signing.ts'
import { timestampOf } from '../device/time.ts'

const DAY = 24 * 60 * 60_000
const MAX_AGE = 7 * DAY
const MAX_AHEAD = 5 * 60_000
const MAX_REQUESTS = 10
const MAX_LABEL = 80

/** A refusal: the status and message the node answers with. */
export interface Refusal {
  status: number
  message: string
}

const refuse = (status: number, message: string): Refusal => ({ status, message })

export const isRefusal = (value: unknown): value is Refusal => typeof value === 'object' && value !== null && 'status' in value && 'message' in value

export interface MockDevices {
  list: () => DeviceList
  /** The ids of the account's devices, in any org. */
  deviceIds: () => ReadonlySet<string>
  /** Keeps a request to add a device, as `POST /api/devices/requests` does. */
  request: (body: unknown) => Promise<DeviceRequest | Refusal>
  /** Applies a change signed by one of the account's devices. */
  change: (body: unknown) => Promise<Refusal | undefined>
  /** Checks that the envelope is the account's, fresh, and signed by a device its org holds. */
  checkEnvelope: (org: string, team: string, signed: unknown) => Promise<Refusal | undefined>
  /** What `agency device-add` does on the node's terminal: adds the first device, after the codes were compared. */
  addFromTerminal: (pairingCode: string) => boolean
}

const isSigned = (value: unknown): value is Signed<Record<string, unknown>> =>
  typeof value === 'object' && value !== null && 'body' in value && 'signer' in value && 'signature' in value && typeof value.signer === 'string' && typeof value.signature === 'string' && typeof value.body === 'object' && value.body !== null

async function verifies(device: Device, kind: SignedKind, signed: Signed<Record<string, unknown>>): Promise<boolean> {
  try {
    const key = await crypto.subtle.importKey('raw', publicKeyBytes(device.publicKey), 'Ed25519', false, ['verify'])
    return await crypto.subtle.verify('Ed25519', key, fromBase64(signed.signature), signedText(kind, signed.body as Json))
  } catch {
    return false
  }
}

const isFresh = (at: unknown, now: number): boolean => typeof at === 'string' && Date.parse(at) >= now - MAX_AGE && Date.parse(at) <= now + MAX_AHEAD

export function createMockDevices(orgs: readonly string[], account: string, initial: ReadonlyArray<Pick<Device, 'id' | 'label' | 'publicKey'>> = []): MockDevices {
  const seeded: readonly Device[] = initial.map((one) => ({ ...one, addedAt: timestampOf(new Date()) }))
  let devices = new Map<string, readonly Device[]>(orgs.map((org) => [org, seeded]))
  let requests: readonly DeviceRequest[] = []

  const list = (): DeviceList => ({ orgs: [...devices].map(([org, held]) => ({ org, devices: [...held] })), requests: [...requests] })
  const holder = (org: string, id: string): Device | undefined => devices.get(org)?.find((one) => one.id === id)
  const add = (org: string, device: Device): void => void (devices = new Map([...devices, [org, [...(devices.get(org) ?? []), device]]]))

  const request: MockDevices['request'] = async (body) => {
    if (typeof body !== 'object' || body === null || !('label' in body) || !('publicKey' in body) || typeof body.label !== 'string' || typeof body.publicKey !== 'string') return refuse(400, 'The request is malformed.')
    const label = body.label.trim()
    if (label.length < 1 || label.length > MAX_LABEL) return refuse(400, 'The label is 1 to 80 characters.')
    let code: string
    try {
      code = await pairingCodeOf(body.publicKey)
    } catch {
      return refuse(400, 'That is not a public key.')
    }
    const existing = requests.find((one) => one.publicKey === body.publicKey)
    if (existing !== undefined) return existing
    if (requests.length >= MAX_REQUESTS) return refuse(429, 'Too many requests are waiting.')
    const asked: DeviceRequest = { label, publicKey: body.publicKey, deviceId: deviceIdOf(code), pairingCode: code, requestedAt: timestampOf(new Date()) }
    requests = [...requests, asked]
    return asked
  }

  const change: MockDevices['change'] = async (body) => {
    if (!isSigned(body)) return refuse(400, 'The change is malformed.')
    const { org, account: changed, op, at } = body.body as { org?: string; account?: string; op?: string; at?: string; device?: unknown }
    if (org === undefined || !devices.has(org)) return refuse(404, 'No such organization.')
    if (changed !== account) return refuse(403, 'That is not this node’s account.')
    const signer = holder(org, body.signer)
    if (signer === undefined) return refuse(403, 'Only one of the account’s devices can sign a change; the first device is added from the node’s terminal.')
    if (!(await verifies(signer, 'device', body))) return refuse(401, 'The signature does not match.')
    if (!isFresh(at, Date.now())) return refuse(400, 'The change is too old or from the future.')
    if (op === 'add') {
      const device = (body.body as { device: Device }).device
      if ((devices.get(org) ?? []).some((one) => one.id === device.id || one.publicKey === device.publicKey)) return refuse(409, 'That device is already added.')
      add(org, device)
      requests = requests.filter((one) => one.publicKey !== device.publicKey)
      return undefined
    }
    if (op === 'remove') {
      const id = (body.body as { device: string }).device
      if (holder(org, id) === undefined) return refuse(409, 'There is no such device.')
      devices = new Map([...devices, [org, (devices.get(org) ?? []).filter((one) => one.id !== id)]])
      return undefined
    }
    return refuse(400, 'The change is malformed.')
  }

  const checkEnvelope: MockDevices['checkEnvelope'] = async (org, team, signed) => {
    if (!isSigned(signed)) return refuse(400, 'The envelope is malformed.')
    const envelope = signed.body as Partial<Envelope>
    if (envelope.account !== account) return refuse(403, 'That is not this node’s account.')
    if (envelope.org !== org || envelope.team !== team) return refuse(400, 'The envelope is for another team.')
    const signer = holder(org, signed.signer)
    if (signer === undefined || !(await verifies(signer, 'envelope', signed))) return refuse(401, 'The device signature does not match.')
    return isFresh(envelope.at, Date.now()) ? undefined : refuse(400, 'The envelope is too old or from the future.')
  }

  const addFromTerminal: MockDevices['addFromTerminal'] = (pairingCode) => {
    const asked = requests.find((one) => one.pairingCode === pairingCode)
    if (asked === undefined) return false
    const device: Device = { id: asked.deviceId, label: asked.label, publicKey: asked.publicKey, addedAt: timestampOf(new Date()) }
    for (const org of orgs) if (!(devices.get(org) ?? []).some((one) => one.publicKey === device.publicKey)) add(org, device)
    requests = requests.filter((one) => one !== asked)
    return true
  }

  const deviceIds: MockDevices['deviceIds'] = () => new Set([...devices.values()].flatMap((held) => held.map((one) => one.id)))

  return { list, deviceIds, request, change, checkEnvelope, addFromTerminal }
}

export type { DeviceChange }
