import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { deviceKeyOf, generateKeyPair } from '../device/deviceKey'
import { deviceIdOf } from '../device/fingerprint'
import { renderApp } from '../test/render'
import { createTestNode, TEST_DEVICE_ID } from '../test/testDevice'

const other = await deviceKeyOf(await generateKeyPair())
const OTHER_ID = deviceIdOf(other.pairingCode)
const TOKEN = 'ab'.repeat(32)
const ENDPOINT = 'https://fcm.googleapis.com/fcm/send/abc'

/** A node that ties registrations to devices, with two devices, and what is registered at it through its own API as the apps and browsers do. */
async function nodeWith(registrations: ReadonlyArray<'iphone-of-laptop' | 'old-ipad' | 'browser-of-this'>, options: { pushDevice?: boolean } = { pushDevice: true }) {
  const node = createTestNode({ webPush: true, pushDevice: options.pushDevice === true, devices: [{ id: OTHER_ID, label: 'Work laptop', publicKey: other.publicKey }] })
  const api = createApiClient((request) => node.handle(request))
  if (registrations.includes('iphone-of-laptop')) await api.registerPushToken({ token: TOKEN, environment: 'production', label: 'Alice’s iPhone', ...(options.pushDevice === true ? { device: OTHER_ID } : {}) })
  if (registrations.includes('old-ipad')) await api.registerPushToken({ token: 'cd'.repeat(32), environment: 'production', label: 'Old iPad' })
  if (registrations.includes('browser-of-this')) await api.registerPushSubscription({ endpoint: ENDPOINT, p256dh: 'P'.repeat(87), auth: 'A'.repeat(22), label: 'Firefox on Linux', ...(options.pushDevice === true ? { device: TEST_DEVICE_ID } : {}) })
  return node
}

describe('where the node sends notices', () => {
  it('lists each browser and app with when it registered and the device it is tied to, or none', async () => {
    const node = await nodeWith(['iphone-of-laptop', 'old-ipad', 'browser-of-this'])
    renderApp({ path: '/devices', node, enrolled: true })
    const card = within(await screen.findByRole('region', { name: 'Where notices are sent' }))
    expect(await card.findByText('Alice’s iPhone')).toBeInTheDocument()
    expect(card.getByText('Tied to Work laptop.')).toBeInTheDocument()
    expect(card.getByText('Tied to Browser.')).toBeInTheDocument()
    expect(card.getByText('Tied to no device. It stays until you stop it.')).toBeInTheDocument()
    expect(card.getAllByText(/iPhone or Mac app · registered/)).toHaveLength(2)
    expect(card.getByText(/Browser · registered/)).toBeInTheDocument()
  })

  it('says a registration names a device that is not the account’s any more, when the list of devices no longer has it', async () => {
    const node = await nodeWith(['iphone-of-laptop'])
    // The node's own list of devices no longer holds the laptop, while a registration still names it.
    const staleDevices = createApiClient(async (request) => {
      const reply = await node.handle(request)
      if (new URL(request.url, 'http://mock.local').pathname !== '/api/devices' || request.method !== 'GET') return reply
      const list = (await reply.json()) as { orgs: Array<{ devices: Array<{ id: string }> }> }
      return new Response(JSON.stringify({ ...list, orgs: list.orgs.map((org) => ({ ...org, devices: org.devices.filter((device) => device.id !== OTHER_ID) })) }), { headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/devices', node, client: staleDevices, enrolled: true })
    const card = within(await screen.findByRole('region', { name: 'Where notices are sent' }))
    expect(await card.findByText(`Tied to a device that is not yours any more (${OTHER_ID}).`)).toBeInTheDocument()
  })

  it('lists the newest registration first', async () => {
    const node = await nodeWith(['iphone-of-laptop', 'old-ipad'])
    // The node lists the iPhone first, and gives its registration a day older than the iPad's.
    const older = createApiClient(async (request) => {
      const reply = await node.handle(request)
      if (new URL(request.url, 'http://mock.local').pathname !== '/api/push/tokens' || request.method !== 'GET') return reply
      const list = (await reply.json()) as { tokens: Array<{ label: string; registeredAt: string }> }
      return new Response(JSON.stringify({ tokens: list.tokens.map((token) => (token.label === 'Alice’s iPhone' ? { ...token, registeredAt: new Date(Date.parse(token.registeredAt) - 86_400_000).toISOString() } : token)) }), { headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/devices', node, client: older, enrolled: true })
    const card = within(await screen.findByRole('region', { name: 'Where notices are sent' }))
    await card.findByText('Old iPad')
    expect(card.getAllByRole('listitem').map((row) => within(row).getAllByText(/./)[0]?.textContent)).toEqual(['Old iPad', 'Alice’s iPhone'])
  })

  it('says nothing of a device for a node that does not tie registrations to devices', async () => {
    const node = await nodeWith(['old-ipad'], { pushDevice: false })
    renderApp({ path: '/devices', node, enrolled: true })
    const card = within(await screen.findByRole('region', { name: 'Where notices are sent' }))
    expect(await card.findByText('Old iPad')).toBeInTheDocument()
    expect(card.queryByText(/Tied to/)).not.toBeInTheDocument()
  })

  it('says it sends nowhere, not that it failed, when the node has no Web Push to list', async () => {
    renderApp({ path: '/devices', node: createTestNode(), enrolled: true })
    const card = within(await screen.findByRole('region', { name: 'Where notices are sent' }))
    expect(await card.findByText('This node sends notices nowhere.')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('says it sends nowhere when nothing is registered', async () => {
    renderApp({ path: '/devices', node: await nodeWith([]), enrolled: true })
    const card = within(await screen.findByRole('region', { name: 'Where notices are sent' }))
    expect(await card.findByText('This node sends notices nowhere.')).toBeInTheDocument()
  })

  it('draws nothing but that it asks while the node has not answered, and says a failure', async () => {
    const node = await nodeWith(['old-ipad'])
    const failing = createApiClient((request) => (new URL(request.url, 'http://mock.local').pathname === '/api/push/tokens' && request.method === 'GET' ? Promise.resolve(new Response(JSON.stringify({ error: 'The node is busy.' }), { status: 409, headers: { 'content-type': 'application/json' } })) : node.handle(request)))
    renderApp({ path: '/devices', node, client: failing, enrolled: true })
    const card = within(await screen.findByRole('region', { name: 'Where notices are sent' }))
    expect(await card.findByText('Could not read where notices are sent: The node is busy.')).toBeInTheDocument()
    expect(card.queryByText('This node sends notices nowhere.')).not.toBeInTheDocument()
  })

  it('stops notices to an app only after it asks and is confirmed, and the node forgets it', async () => {
    const user = userEvent.setup()
    const node = await nodeWith(['old-ipad', 'browser-of-this'])
    renderApp({ path: '/devices', node, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Stop notices to Old iPad' }))
    const panel = await screen.findByRole('group', { name: 'Stop notices to Old iPad?' })
    expect(within(panel).getByText(/Old iPad \(registered .*\) will no longer get notices from this node/)).toBeInTheDocument()
    expect(node.pushTokenEntries()).toHaveLength(1)
    await user.click(within(panel).getByRole('button', { name: 'Stop notices' }))
    expect(await screen.findByText('Old iPad no longer gets notices.')).toBeInTheDocument()
    expect(node.pushTokenEntries()).toHaveLength(0)
    expect(node.pushSubscriptions()).toHaveLength(1)
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Stop notices to Old iPad' })).not.toBeInTheDocument())
  })

  it('stops notices to a browser the same way, and does nothing when the person cancels', async () => {
    const user = userEvent.setup()
    const node = await nodeWith(['browser-of-this', 'old-ipad'])
    renderApp({ path: '/devices', node, enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Stop notices to Firefox on Linux' }))
    await user.click(await screen.findByRole('button', { name: 'Cancel' }))
    expect(node.pushSubscriptions()).toHaveLength(1)
    await user.click(screen.getByRole('button', { name: 'Stop notices to Firefox on Linux' }))
    await user.click(await screen.findByRole('button', { name: 'Stop notices' }))
    expect(await screen.findByText('Firefox on Linux no longer gets notices.')).toBeInTheDocument()
    expect(node.pushSubscriptions()).toHaveLength(0)
    expect(node.pushTokenEntries()).toHaveLength(1)
  })

  it('says it in Japanese', async () => {
    const user = userEvent.setup()
    const node = await nodeWith(['old-ipad'])
    renderApp({ path: '/devices', node, enrolled: true, locale: 'ja' })
    const card = within(await screen.findByRole('region', { name: '通知の送り先' }))
    expect(await card.findByText('どの端末にも紐づいていません。止めるまで残ります。')).toBeInTheDocument()
    await user.click(card.getByRole('button', { name: 'Old iPadへの通知を止める' }))
    await user.click(await screen.findByRole('button', { name: '通知を止める' }))
    expect(await screen.findByText('Old iPadへの通知を止めました。')).toBeInTheDocument()
  })

  it('is not drawn for a hosted member, who is not offered the owner’s list of apps', async () => {
    renderApp({ path: '/devices', node: createTestNode({ guest: { member: 'carol', label: 'Carol' } }), enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Devices' })
    expect(screen.queryByRole('region', { name: 'Where notices are sent' })).not.toBeInTheDocument()
  })
})
