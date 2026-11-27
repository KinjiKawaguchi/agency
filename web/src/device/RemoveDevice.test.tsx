import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'
import { deviceKeyOf, generateKeyPair } from './deviceKey'
import { deviceIdOf } from './fingerprint'

const other = await deviceKeyOf(await generateKeyPair())
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
    await user.click(await screen.findByRole('button', { name: 'Remove Work laptop' }))
    await user.click(await screen.findByRole('button', { name: 'Remove this device' }))
    expect(await screen.findByText('Work laptop is removed. It can never sign again.')).toBeInTheDocument()
    expect(await listedBy(node)).toEqual([['acme', ['Browser']]])
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
    expect(within(panel).getByText(/This is the device you are using\. After it is removed, this browser can no longer sign, and its key cannot be added again\./)).toBeInTheDocument()
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
