import { addChangeFor, signChange } from '../device/changes'
import { deviceKeyOf, generateKeyPair, type DeviceKey } from '../device/deviceKey'
import { buildEnvelope, newEnvelopeId, signEnvelope } from '../device/envelope'
import { createApiClient } from '../api/client'
import type { DecisionAnswer } from '../api/clientApi'
import { createMockNode, MOCK_TOKEN } from './node'

const ANSWER: DecisionAnswer = { org: 'acme', team: 'app', answerWith: { kind: 'answer', target: 'r-20261124-113000-c3d4' }, title: 'A request', text: 'Go.' }

async function nodeWithDevice() {
  const node = createMockNode()
  const api = createApiClient((request) => node.handle(request))
  const key = await deviceKeyOf(await generateKeyPair())
  const asked = await api.requestDevice({ label: 'Laptop', publicKey: key.publicKey })
  node.addDeviceFromTerminal(asked.pairingCode)
  return { node, api, key, deviceId: asked.deviceId }
}

const envelopeFor = async (key: DeviceKey, deviceId: string, at = new Date(), account = 'alice') =>
  signEnvelope(key, deviceId, buildEnvelope({ answer: ANSWER, account, id: newEnvelopeId(), at }))

describe('the mock node’s device routes', () => {
  it('keeps a request with the pairing code and id the protocol derives, and keeps one request per key', async () => {
    const node = createMockNode()
    const api = createApiClient((request) => node.handle(request))
    const key = await deviceKeyOf(await generateKeyPair())
    const asked = await api.requestDevice({ label: 'Laptop', publicKey: key.publicKey })
    expect(asked).toMatchObject({ pairingCode: key.pairingCode, deviceId: `d-${key.pairingCode.replaceAll('-', '')}` })
    expect(await api.requestDevice({ label: 'Again', publicKey: key.publicKey })).toEqual(asked)
    expect((await api.devices()).requests).toHaveLength(1)
  })

  it('refuses a request without a usable label or key', async () => {
    const api = createApiClient((request) => createMockNode().handle(request))
    await expect(api.requestDevice({ label: ' ', publicKey: 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=' })).rejects.toThrow('label')
    await expect(api.requestDevice({ label: 'x'.repeat(81), publicKey: 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=' })).rejects.toThrow('label')
    await expect(api.requestDevice({ label: 'Laptop', publicKey: 'nope' })).rejects.toThrow('public key')
  })

  it('accepts a decision signed by a device its org holds', async () => {
    const { api, key, deviceId, node } = await nodeWithDevice()
    const reply = await api.decideSigned('acme', 'app', { envelope: await envelopeFor(key, deviceId), title: 'A request' })
    expect(reply.outcome).toBe('applied')
    expect(node.envelopes()).toHaveLength(1)
  })

  it('sends the notes for the log trimmed, and the choice’s label only with a choice', async () => {
    const { api, key, deviceId } = await nodeWithDevice()
    const withChoice = { ...ANSWER, choice: 'approve' }
    await api.decideSigned('acme', 'app', { envelope: await signEnvelope(key, deviceId, buildEnvelope({ answer: withChoice, account: 'alice', id: newEnvelopeId(), at: new Date() })), title: `  ${'t'.repeat(300)}`, choiceLabel: ' Approve ' })
    const [entry] = (await api.decisions(1)).entries
    expect(entry?.title).toHaveLength(200)
    expect(entry?.choiceLabel).toBe('Approve')

    const second = await nodeWithDevice()
    await second.api.decideSigned('acme', 'app', { envelope: await envelopeFor(second.key, second.deviceId), choiceLabel: 'Approve' })
    expect((await second.api.decisions(1)).entries[0]).not.toHaveProperty('choiceLabel')
  })

  it('refuses a decision whose body was changed after it was signed', async () => {
    const { api, key, deviceId } = await nodeWithDevice()
    const signed = await envelopeFor(key, deviceId)
    await expect(api.decideSigned('acme', 'app', { envelope: { ...signed, body: { ...signed.body, payload: { text: 'Something else.' } } } })).rejects.toThrow('signature')
  })

  it('refuses a decision signed by a key that is not a device of the org', async () => {
    const { api, deviceId } = await nodeWithDevice()
    const stranger = await deviceKeyOf(await generateKeyPair())
    await expect(api.decideSigned('acme', 'app', { envelope: await envelopeFor(stranger, deviceId) })).rejects.toThrow('signature')
    await expect(api.decideSigned('acme', 'app', { envelope: await envelopeFor(stranger, 'd-000000000000') })).rejects.toThrow('signature')
  })

  it('refuses a decision for another account, another team, or at the wrong time', async () => {
    const { api, key, deviceId } = await nodeWithDevice()
    await expect(api.decideSigned('acme', 'app', { envelope: await envelopeFor(key, deviceId, new Date(), 'bob') })).rejects.toThrow('account')
    await expect(api.decideSigned('acme', 'site', { envelope: await envelopeFor(key, deviceId) })).rejects.toThrow('another team')
    await expect(api.decideSigned('acme', 'app', { envelope: await envelopeFor(key, deviceId, new Date(Date.now() - 8 * 86_400_000)) })).rejects.toThrow('too old')
    await expect(api.decideSigned('acme', 'app', { envelope: await envelopeFor(key, deviceId, new Date(Date.now() + 10 * 60_000)) })).rejects.toThrow('future')
  })

  it('takes a device change only from a device the org holds, and never lets a first device in through the API', async () => {
    const node = createMockNode()
    const api = createApiClient((request) => node.handle(request))
    const first = await deviceKeyOf(await generateKeyPair())
    const asked = await api.requestDevice({ label: 'First', publicKey: first.publicKey })
    const change = addChangeFor(asked, 'acme', 'alice', new Date())
    await expect(api.submitDeviceChange(await signChange(first, asked.deviceId, change))).rejects.toThrow('terminal')
    expect((await api.devices()).orgs[0]?.devices).toEqual([])
  })

  it('adds a device when an existing one signs the change, and refuses a signature that does not match', async () => {
    const { api, key, deviceId } = await nodeWithDevice()
    const second = await deviceKeyOf(await generateKeyPair())
    const asked = await api.requestDevice({ label: 'Phone', publicKey: second.publicKey })
    const change = addChangeFor(asked, 'acme', 'alice', new Date())
    const forged = await signChange(second, deviceId, change)
    await expect(api.submitDeviceChange(forged)).rejects.toThrow('signature')
    expect(await api.submitDeviceChange(await signChange(key, deviceId, change))).toEqual({ outcome: 'applied' })
    const list = await api.devices()
    expect(list.orgs[0]?.devices.map((one) => one.label)).toEqual(['Laptop', 'Phone'])
    expect(list.requests).toEqual([])
  })

  it('refuses a write without the page’s token', async () => {
    const node = createMockNode()
    const reply = await node.handle(new Request('http://mock.local/api/devices/requests', { method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}' }))
    expect(reply.status).toBe(403)
    expect(MOCK_TOKEN).not.toBe('')
  })
})
