import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { renderApp } from '../test/render'
import { createTestNode, TEST_DEVICE_ID, testKeyStore } from '../test/testDevice'
import { deviceKeyOf, generateKeyPair } from './deviceKey'
import { deviceIdOf } from './fingerprint'

const other = await deviceKeyOf(await generateKeyPair())
const tablet = await deviceKeyOf(await generateKeyPair())
const OTHER_ID = deviceIdOf(other.pairingCode)
const withLaptop = (options: Parameters<typeof createTestNode>[0] = {}) =>
  createTestNode({ ...options, devices: [{ id: OTHER_ID, label: 'Work laptop', publicKey: other.publicKey }] })

const listedBy = async (node: ReturnType<typeof createTestNode>): Promise<Array<[string, string[]]>> =>
  (await createApiClient((request) => node.handle(request)).devices()).orgs.map((one) => [one.org, one.devices.map((device) => device.label)])

describe('removing one of the account’s devices', () => {
  it('asks first, naming the device by its label and its pairing code, and says it can never sign again', async () => {
    const user = userEvent.setup()
    const node = withLaptop()
    renderApp({ path: '/devices', node, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    const panel = await screen.findByRole('group', { name: 'Remove Work laptop?' })
    expect(await within(panel).findByText(`Work laptop (pairing code ${other.pairingCode}) can never sign again.`)).toBeInTheDocument()
    expect(within(panel).queryByText(/This is the device you are using/)).not.toBeInTheDocument()
    // Nothing is removed until it is confirmed.
    expect(await listedBy(node)).toEqual([['acme', ['Browser', 'Work laptop']]])
  })

  it('removes it when confirmed, as this browser signs it, and the device leaves the list when the node stops listing it', async () => {
    const user = userEvent.setup()
    const node = withLaptop()
    renderApp({ path: '/devices', node, enrolled: true })
    const before = (await screen.findByLabelText('Pairing code of this device')).textContent
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    await user.click(await screen.findByRole('button', { name: 'Remove this device' }))
    expect(await screen.findByText('Work laptop is removed. It can never sign again.')).toBeInTheDocument()
    expect(await listedBy(node)).toEqual([['acme', ['Browser']]])
    // Another device's removal leaves this browser's own key alone.
    expect(screen.getByLabelText('Pairing code of this device').textContent).toBe(before)
    expect(screen.queryByRole('button', { name: 'Add this device' })).not.toBeInTheDocument()
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Remove Work laptop' })).not.toBeInTheDocument())
  })

  it('does nothing when the person cancels', async () => {
    const user = userEvent.setup()
    const node = withLaptop()
    renderApp({ path: '/devices', node, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    await user.click(await screen.findByRole('button', { name: 'Cancel' }))
    expect(screen.queryByRole('group', { name: 'Remove Work laptop?' })).not.toBeInTheDocument()
    expect(await listedBy(node)).toEqual([['acme', ['Browser', 'Work laptop']]])
  })

  it('removes a device from every org that lists it with one confirmation, and says so before', async () => {
    const user = userEvent.setup()
    const node = withLaptop({ deviceOrgs: ['acme', 'club'] })
    renderApp({ path: '/devices', node, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    const panel = await screen.findByRole('group', { name: 'Remove Work laptop?' })
    const orgs = (await listedBy(node)).map(([org]) => org)
    expect(orgs.length).toBeGreaterThan(1)
    expect(within(panel).getByText(`It is removed from ${orgs.join(', ')}.`)).toBeInTheDocument()
    await user.click(within(panel).getByRole('button', { name: 'Remove this device' }))
    await screen.findByText('Work laptop is removed. It can never sign again.')
    expect((await listedBy(node)).every(([, labels]) => !labels.includes('Work laptop'))).toBe(true)
  })

  it('says separately what removing this very device means: this browser can no longer sign', async () => {
    const user = userEvent.setup()
    const node = withLaptop()
    renderApp({ path: '/devices', node, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Browser' }))
    const panel = await screen.findByRole('group', { name: 'Remove Browser?' })
    expect(within(panel).getByText(/This is the device you are using\. After it is removed, this browser can no longer sign with this key, and the key cannot be added again\. The browser then makes a new key, which has to be added as a new device\./)).toBeInTheDocument()
  })

  it('makes a new key when this browser removes itself, and offers to add that one, never the revoked key', async () => {
    const user = userEvent.setup()
    const node = withLaptop()
    const kept = vi.fn(() => Promise.resolve())
    renderApp({ path: '/devices', node, enrolled: true, keyStore: { load: testKeyStore().load, save: kept } })
    const before = (await screen.findByLabelText('Pairing code of this device')).textContent
    await user.click(await screen.findByRole('button', { name: 'Remove Browser' }))
    await user.click(await screen.findByRole('button', { name: 'Remove this device' }))
    expect(await screen.findByText(/This device is removed\. This browser made a new key, which is not added yet/)).toBeInTheDocument()
    expect(await listedBy(node)).toEqual([['acme', ['Work laptop']]])
    expect(await screen.findByRole('button', { name: 'Add this device' })).toBeInTheDocument()
    expect((await screen.findByLabelText('Pairing code of this device')).textContent).not.toBe(before)
    // The new key is kept in the browser's store, so that it is the one the next visit finds.
    expect(kept).toHaveBeenCalledTimes(1)
  })

  it('keeps its key while the removal of this browser is only held for the root key’s node', async () => {
    const user = userEvent.setup()
    const node = withLaptop({ pendingRoot: true })
    renderApp({ path: '/devices', node, enrolled: true })
    const before = (await screen.findByLabelText('Pairing code of this device')).textContent
    await user.click(await screen.findByRole('button', { name: 'Remove Browser' }))
    await user.click(await screen.findByRole('button', { name: 'Remove this device' }))
    await screen.findByText(/Removal sent here/)
    expect(screen.getByLabelText('Pairing code of this device').textContent).toBe(before)
    expect(screen.queryByRole('button', { name: 'Add this device' })).not.toBeInTheDocument()
  })

  it('does not offer to remove the only device of the account, and says why', async () => {
    renderApp({ path: '/devices' })
    expect(await screen.findByText(/Browser is your only device in acme, so it cannot be removed here: no device there could sign after it\./)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Remove Browser' })).not.toBeInTheDocument()
    expect(screen.getByText(/agency device-add adds one/)).toBeInTheDocument()
  })

  it('says a removal the root key’s node holds is sent, not done, and leaves the device listed', async () => {
    const user = userEvent.setup()
    const node = withLaptop({ pendingRoot: true })
    renderApp({ path: '/devices', node, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    await user.click(await screen.findByRole('button', { name: 'Remove this device' }))
    expect(await screen.findByText(/Removal sent here; it takes effect once the root key’s node signs it: run agency root-sign there\./)).toBeInTheDocument()
    expect(screen.queryByText('Work laptop is removed. It can never sign again.')).not.toBeInTheDocument()
    expect(await listedBy(node)).toEqual([['acme', ['Browser', 'Work laptop']]])
  })

  it('says the node’s refusal, and the device stays', async () => {
    const user = userEvent.setup()
    const node = withLaptop()
    const refusing = createApiClient(async (request) =>
      new URL(request.url, 'http://mock.local').pathname === '/api/devices/changes'
        ? new Response(JSON.stringify({ error: 'The root refused it.' }), { status: 409, headers: { 'content-type': 'application/json' } })
        : node.handle(request),
    )
    renderApp({ path: '/devices', node, client: refusing, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    await user.click(await screen.findByRole('button', { name: 'Remove this device' }))
    expect(await screen.findByText('Could not remove the device: The root refused it.')).toBeInTheDocument()
    expect(await listedBy(node)).toEqual([['acme', ['Browser', 'Work laptop']]])
  })

  it('reads the devices again before it signs, so that the rule of the last device is the node’s now, not a few seconds ago', async () => {
    const user = userEvent.setup()
    const node = withLaptop()
    let isReduced = false
    let changes = 0
    // After the person has been asked, the other device goes: the list the node gives is now one device.
    const client = createApiClient(async (request) => {
      const path = new URL(request.url, 'http://mock.local').pathname
      if (path === '/api/devices/changes') changes += 1
      const reply = await node.handle(request)
      if (!isReduced || path !== '/api/devices' || request.method !== 'GET') return reply
      const list = (await reply.json()) as { orgs: Array<{ devices: Array<{ id: string }> }> }
      return new Response(JSON.stringify({ ...list, orgs: list.orgs.map((org) => ({ ...org, devices: org.devices.filter((one) => one.id !== OTHER_ID) })) }), { headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/devices', node, client, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Browser' }))
    const panel = await screen.findByRole('group', { name: 'Remove Browser?' })
    isReduced = true
    await user.click(within(panel).getByRole('button', { name: 'Remove this device' }))
    expect(await screen.findByText(/Could not remove the device: Browser is your only device in acme/)).toBeInTheDocument()
    expect(changes).toBe(0)
    expect(await listedBy(node)).toEqual([['acme', ['Browser', 'Work laptop']]])
  })

  it('signs the removal for the orgs the node lists now, not those it listed when the person was asked', async () => {
    const user = userEvent.setup()
    const node = withLaptop({ deviceOrgs: ['acme', 'club'] })
    let isFull = false
    // When the person is asked the list has one org; by the time they confirm, the node lists the device in two.
    const client = createApiClient(async (request) => {
      const reply = await node.handle(request)
      if (isFull || new URL(request.url, 'http://mock.local').pathname !== '/api/devices' || request.method !== 'GET') return reply
      const list = (await reply.json()) as { orgs: Array<{ org: string }> }
      return new Response(JSON.stringify({ ...list, orgs: list.orgs.filter((one) => one.org === 'acme') }), { headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/devices', node, client, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    const panel = await screen.findByRole('group', { name: 'Remove Work laptop?' })
    isFull = true
    await user.click(within(panel).getByRole('button', { name: 'Remove this device' }))
    await screen.findByText('Work laptop is removed. It can never sign again.')
    expect((await listedBy(node)).every(([, labels]) => !labels.includes('Work laptop'))).toBe(true)
  })

  describe('while decisions are kept in this browser for sending', () => {
    const kept = (signer: string) => ({
      id: 'e-1', org: 'acme', team: 'app', itemKey: 'acme:app:r-1', itemTitle: 'Add a second frontend agent', text: '', draft: { text: '', choice: null },
      envelope: { body: { id: 'e-1' }, signer, signature: 'c2ln' }, createdAt: '2026-11-27T10:00:00Z', attempts: 1, phase: 'failed', reason: 'The node is busy.',
    })
    afterEach(() => window.localStorage.removeItem('agency.outbox'))

    it('does not offer to remove this device, whose key signed them, and says to send or discard them first', async () => {
      window.localStorage.setItem('agency.outbox', JSON.stringify([kept(TEST_DEVICE_ID)]))
      renderApp({ path: '/devices', node: withLaptop(), enrolled: true })
      expect(await screen.findByText('1 decision has not been sent yet, and removing this device would stop it for good. Send or discard it in the tray first.')).toBeInTheDocument()
      expect(screen.queryByRole('button', { name: 'Remove Browser' })).not.toBeInTheDocument()
      // Another device's removal is not touched by them.
      expect(await screen.findByRole('button', { name: 'Remove Work laptop' })).toBeInTheDocument()
    })

    it('offers it when what is kept was signed by another device', async () => {
      window.localStorage.setItem('agency.outbox', JSON.stringify([kept(OTHER_ID)]))
      renderApp({ path: '/devices', node: withLaptop(), enrolled: true })
      expect(await screen.findByRole('button', { name: 'Remove Browser' })).toBeInTheDocument()
    })
  })

  it('says in the confirmation how many decisions the node holds as saved, not delivered, and that those the device signed will be refused', async () => {
    const user = userEvent.setup()
    // The sample log has one decision the node holds for a team that is asleep.
    renderApp({ path: '/devices', node: withLaptop(), enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    const panel = await screen.findByRole('group', { name: 'Remove Work laptop?' })
    expect(await within(panel).findByText('1 decision is saved, not delivered. Any that Work laptop signed will be refused once it is removed.')).toBeInTheDocument()
  })

  it('says it of the ones made in this browser when it is this device that goes, and still lets it go', async () => {
    const user = userEvent.setup()
    const node = withLaptop()
    renderApp({ path: '/devices', node, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Browser' }))
    const panel = await screen.findByRole('group', { name: 'Remove Browser?' })
    expect(await within(panel).findByText('1 decision is saved, not delivered. The ones made in this browser will be refused once it is removed.')).toBeInTheDocument()
    await user.click(within(panel).getByRole('button', { name: 'Remove this device' }))
    expect(await screen.findByText(/This device is removed\./)).toBeInTheDocument()
    expect(await listedBy(node)).toEqual([['acme', ['Work laptop']]])
  })

  it('asks the node for as many entries of the log as it gives, and says "at least" when the answer is full', async () => {
    const user = userEvent.setup()
    const node = withLaptop()
    const asked: string[] = []
    // A log that is full (200 entries), each one held by the node: the count read is a floor, not the number there are.
    const client = createApiClient(async (request) => {
      const url = new URL(request.url, 'http://mock.local')
      const reply = await node.handle(request)
      if (url.pathname !== '/api/decisions' || request.method !== 'GET') return reply
      asked.push(url.search)
      const log = (await reply.json()) as { entries: Array<{ id: string; state: { kind: string } }> }
      const [first] = log.entries
      const many = Array.from({ length: 200 }, (_, index) => ({ ...first, id: `e-${index}`, state: { kind: 'queued', attempts: 1 } }))
      return new Response(JSON.stringify({ entries: many }), { headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/devices', node, client, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    const panel = await screen.findByRole('group', { name: 'Remove Work laptop?' })
    expect(await within(panel).findByText('At least 200 decisions are saved, not delivered. Any that Work laptop signed will be refused once it is removed.')).toBeInTheDocument()
    expect(asked.some((search) => search.includes('limit=200'))).toBe(true)
  })

  it('says nothing of undelivered decisions when the node holds none', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/devices', node: withLaptop({ earlierDecisions: false }), enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    const panel = await screen.findByRole('group', { name: 'Remove Work laptop?' })
    await within(panel).findByText(/can never sign again\./)
    expect(within(panel).queryByText(/saved, not delivered/)).not.toBeInTheDocument()
  })

  it('says where the device was removed and where it is still listed, and does not say it can never sign again of the latter', async () => {
    const user = userEvent.setup()
    // A second other device keeps the laptop from being the last one in club.
    const node = createTestNode({ deviceOrgs: ['acme', 'club'], devices: [{ id: OTHER_ID, label: 'Work laptop', publicKey: other.publicKey }, { id: deviceIdOf(tablet.pairingCode), label: 'Tablet', publicKey: tablet.publicKey }] })
    // This browser is added in acme only: in club it cannot sign the removal, so the laptop stays listed there.
    const client = createApiClient(async (request) => {
      const reply = await node.handle(request)
      if (new URL(request.url, 'http://mock.local').pathname !== '/api/devices' || request.method !== 'GET') return reply
      const list = (await reply.json()) as { orgs: Array<{ org: string; devices: Array<{ id: string }> }> }
      return new Response(JSON.stringify({ ...list, orgs: list.orgs.map((org) => (org.org === 'club' ? { ...org, devices: org.devices.filter((one) => one.id !== TEST_DEVICE_ID) } : org)) }), { headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/devices', node, client, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    await user.click(await screen.findByRole('button', { name: 'Remove this device' }))
    expect(await screen.findByText('Work laptop is removed from acme. It can never sign there again.')).toBeInTheDocument()
    expect(screen.queryByText('Work laptop is removed. It can never sign again.')).not.toBeInTheDocument()
    expect(await screen.findByText(/It is still listed in club/)).toBeInTheDocument()
  })

  it('says, with no button, that a device is listed only where this browser cannot sign, so it cannot be removed from here', async () => {
    const node = createTestNode({ deviceOrgs: ['acme', 'club'], devices: [{ id: OTHER_ID, label: 'Work laptop', publicKey: other.publicKey }, { id: deviceIdOf(tablet.pairingCode), label: 'Tablet', publicKey: tablet.publicKey }] })
    // This browser is added in acme, and the laptop is listed in club only.
    const client = createApiClient(async (request) => {
      const reply = await node.handle(request)
      if (new URL(request.url, 'http://mock.local').pathname !== '/api/devices' || request.method !== 'GET') return reply
      const list = (await reply.json()) as { orgs: Array<{ org: string; devices: Array<{ id: string }> }> }
      const kept = (org: string, id: string): boolean => (org === 'club' ? id !== TEST_DEVICE_ID : id !== OTHER_ID)
      return new Response(JSON.stringify({ ...list, orgs: list.orgs.map((org) => ({ ...org, devices: org.devices.filter((one) => kept(org.org, one.id)) })) }), { headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/devices', node, client, enrolled: true })
    expect(await screen.findByText('It is still listed in club: this device is not added there, so it cannot sign the removal.')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Remove Work laptop' })).not.toBeInTheDocument()
  })

  it('says it in Japanese', async () => {
    const user = userEvent.setup()
    const node = withLaptop()
    renderApp({ path: '/devices', node, enrolled: true, locale: 'ja' })
    await user.click(await screen.findByRole('button', { name: 'Work laptopを削除' }))
    const panel = await screen.findByRole('group', { name: 'Work laptopを削除しますか?' })
    expect(await within(panel).findByText(`Work laptop(照合コード ${other.pairingCode})は、二度と署名できなくなります。`)).toBeInTheDocument()
    await user.click(within(panel).getByRole('button', { name: 'この端末を削除する' }))
    expect(await screen.findByText('Work laptopを削除しました。二度と署名できません。')).toBeInTheDocument()
  })
})
