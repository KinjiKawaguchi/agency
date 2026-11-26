import { focusManager } from '@tanstack/react-query'
import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { createMockNode } from '../mock/node'
import { memoryKeyStore, renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'
import { deviceKeyOf, generateKeyPair, type DeviceKey } from './deviceKey'
import { settledDeviceKey } from './useDeviceKey'

const CODE = /^[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}$/

/** The key this test's browser holds, as the page sees it. */
async function browserKey(store: ReturnType<typeof memoryKeyStore>): Promise<DeviceKey> {
  const held = await settledDeviceKey(store)
  if (held.status !== 'ready') throw new Error('no key')
  return held.key
}

/** What another device would do: ask the node to add it. */
async function otherDeviceAsks(node: ReturnType<typeof createMockNode>, label: string): Promise<DeviceKey> {
  const key = await deviceKeyOf(await generateKeyPair())
  await createApiClient((request) => node.handle(request)).requestDevice({ label, publicKey: key.publicKey })
  return key
}

describe('devices screen', () => {
  it('shows this device’s pairing code, and offers to add it while the node does not know it', async () => {
    renderApp({ path: '/devices', enrolled: false })
    expect((await screen.findByLabelText('Pairing code of this device')).textContent).toMatch(CODE)
    expect(await screen.findByRole('button', { name: 'Add this device' })).toBeInTheDocument()
    expect(screen.getByText(/Decisions and comments need a device of yours\. To add this one, ask for its code below, then confirm that code on the machine that runs the node with “agency device-add”\./)).toBeInTheDocument()
  })

  it('says in Japanese, too, that decisions and comments need a device, and how to add this one', async () => {
    renderApp({ path: '/devices', enrolled: false, locale: 'ja' })
    expect(await screen.findByText(/決定やコメントを送るには、あなたの端末が必要です。.*agency device-add.*承認してください。/)).toBeInTheDocument()
  })

  it('shows the same code on a second visit, because the key is kept', async () => {
    const keyStore = memoryKeyStore()
    const first = renderApp({ path: '/devices', keyStore })
    const code = (await screen.findByLabelText('Pairing code of this device')).textContent
    first.unmount()
    renderApp({ path: '/devices', keyStore })
    expect((await screen.findByLabelText('Pairing code of this device')).textContent).toBe(code)
  })

  it('is reached from the viewer’s own icon, which is the current page there', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/desk' })
    await user.click(await screen.findByRole('link', { name: 'Devices' }))
    expect(await screen.findByRole('heading', { level: 1, name: 'Devices' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Devices' })).toHaveAttribute('aria-current', 'page')
  })

  it('asks the node to add this device with its public key and a name, then waits showing the same code', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    const keyStore = memoryKeyStore()
    renderApp({ path: '/devices', node, keyStore })
    const field = await screen.findByRole('textbox', { name: 'Name for this device' })
    await user.clear(field)
    await user.type(field, 'Work laptop')
    await user.click(screen.getByRole('button', { name: 'Add this device' }))
    expect(await screen.findByText(/Waiting for you to confirm this device/)).toBeInTheDocument()
    const key = await browserKey(keyStore)
    const { requests } = await createApiClient((request) => node.handle(request)).devices()
    expect(requests).toMatchObject([{ label: 'Work laptop', publicKey: key.publicKey, pairingCode: key.pairingCode }])
    expect(screen.getByLabelText('Pairing code of this device')).toHaveTextContent(key.pairingCode)
  })

  it('learns it was added when the node’s terminal confirms it, and then lists its devices', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    const keyStore = memoryKeyStore()
    renderApp({ path: '/devices', node, keyStore })
    await user.click(await screen.findByRole('button', { name: 'Add this device' }))
    await screen.findByText(/Waiting for you to confirm this device/)
    expect(node.addDeviceFromTerminal((await browserKey(keyStore)).pairingCode)).toBe(true)
    window.dispatchEvent(new Event('visibilitychange'))
    expect(await screen.findByText(/This device is added\./)).toBeInTheDocument()
    const list = within(screen.getByRole('list', { name: '' }))
    expect(list.getByText('Browser')).toBeInTheDocument()
    expect(list.getByText('this device')).toBeInTheDocument()
  })

  it('lists a device that asks, with its code to compare, and adds it only when the person says the codes match', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    const keyStore = memoryKeyStore()
    renderApp({ path: '/devices', node, keyStore })
    await user.click(await screen.findByRole('button', { name: 'Add this device' }))
    node.addDeviceFromTerminal((await browserKey(keyStore)).pairingCode)
    window.dispatchEvent(new Event('visibilitychange'))
    await screen.findByText(/This device is added\./)
    expect(screen.getByText('No device is waiting to be added.')).toBeInTheDocument()

    const phone = await otherDeviceAsks(node, 'Phone')
    window.dispatchEvent(new Event('visibilitychange'))
    expect(await screen.findByLabelText('Pairing code of Phone')).toHaveTextContent(phone.pairingCode)
    expect(screen.getByText(/If they differ in any character, do not add it/)).toBeInTheDocument()
    // Nothing is added until the person confirms.
    expect((await createApiClient((request) => node.handle(request)).devices()).orgs[0]?.devices).toHaveLength(1)

    await user.click(screen.getByRole('button', { name: 'The codes match: add it' }))
    expect(await screen.findByText('Phone is added.')).toBeInTheDocument()
    const added = (await createApiClient((request) => node.handle(request)).devices()).orgs[0]?.devices.map((one) => one.label)
    expect(added).toEqual(['Browser', 'Phone'])
    await waitFor(() => expect(screen.queryByLabelText('Pairing code of Phone')).not.toBeInTheDocument())
  })

  it('says why when the node refuses to add the device', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    const keyStore = memoryKeyStore()
    renderApp({ path: '/devices', node, keyStore })
    await user.click(await screen.findByRole('button', { name: 'Add this device' }))
    node.addDeviceFromTerminal((await browserKey(keyStore)).pairingCode)
    window.dispatchEvent(new Event('visibilitychange'))
    await screen.findByText(/This device is added\./)
    await otherDeviceAsks(node, 'Tablet')
    window.dispatchEvent(new Event('visibilitychange'))
    // The device is added elsewhere in the meantime, so the node refuses the second add.
    const tablet = (await createApiClient((request) => node.handle(request)).devices()).requests[0]
    expect(tablet).toBeDefined()
    node.addDeviceFromTerminal(tablet?.pairingCode ?? '')
    await user.click(await screen.findByRole('button', { name: 'The codes match: add it' }))
    expect(await screen.findByText(/Could not add the device: /)).toBeInTheDocument()
  })

  it('says so when this device is added to none of the orgs that lack the new one, and sends nothing', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    const keyStore = memoryKeyStore()
    const real = createApiClient((request) => node.handle(request))
    const mine = await browserKey(keyStore)
    await real.requestDevice({ label: 'Browser', publicKey: mine.publicKey })
    node.addDeviceFromTerminal(mine.pairingCode)
    const phone = await otherDeviceAsks(node, 'Phone')
    // The phone is already in acme, where this device is added, and lacks only `lab`, where this device is not.
    const devices = async () => {
      const list = await real.devices()
      const added = { id: 'd-phone', label: 'Phone', publicKey: phone.publicKey, addedAt: '2026-11-24T12:00:00Z' }
      return { ...list, orgs: [{ org: 'acme', devices: [...(list.orgs[0]?.devices ?? []), added] }, { org: 'lab', devices: [] }] }
    }
    let sent = 0
    renderApp({ path: '/devices', node, keyStore, client: { ...real, devices, submitDeviceChange: (change) => { sent += 1; return real.submitDeviceChange(change) } } })
    await user.click(await screen.findByRole('button', { name: 'The codes match: add it' }))
    expect(await screen.findByText(/Could not add the device: This device is not added to any organization that lacks the new one\./)).toBeInTheDocument()
    expect(sent).toBe(0)
  })

  it('says which part is missing when the browser cannot make device keys', async () => {
    const generateKey = vi.spyOn(crypto.subtle, 'generateKey').mockRejectedValue(new Error('NotSupportedError'))
    try {
      renderApp({ path: '/devices' })
      expect(await screen.findByRole('alert')).toHaveTextContent('This browser cannot make device keys.')
      expect(screen.getByText(/Secure page: .* · WebCrypto: yes · IndexedDB: /)).toBeInTheDocument()
    } finally {
      generateKey.mockRestore()
    }
  })

  it('tells a first-time person what to do while waiting: run the command on the node’s machine and compare the code', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/devices', enrolled: false })
    await user.click(await screen.findByRole('button', { name: 'Add this device' }))
    const steps = await screen.findAllByRole('listitem')
    expect(steps.map((one) => one.textContent)).toEqual([
      'On the machine that runs the node, run “agency device-add”.',
      'Check that the code it shows is exactly the one above.',
      'Confirm there.',
    ])
  })
})

describe('a device list that stays current while the screen is open', () => {
  afterEach(() => focusManager.setFocused(undefined))

  it('shows a request that arrives after the screen has loaded, with its button, without a reload', async () => {
    const node = createTestNode()
    renderApp({ path: '/devices', node, enrolled: true })
    expect(await screen.findByText('No device is waiting to be added.')).toBeInTheDocument()
    const other = await otherDeviceAsks(node, 'Phone')
    expect(await screen.findByLabelText('Pairing code of Phone', undefined, { timeout: 9000 })).toHaveTextContent(other.pairingCode)
    expect(screen.getByRole('button', { name: 'The codes match: add it' })).toBeInTheDocument()
  })

  it('does not read the list while the tab is hidden, and reads it again when the tab is back', async () => {
    const node = createTestNode()
    let reads = 0
    const counted = createApiClient((request) => {
      if (request.method === 'GET' && new URL(request.url, 'http://mock.local').pathname === '/api/devices') reads += 1
      return node.handle(request)
    })
    renderApp({ path: '/devices', node, enrolled: true, client: counted })
    expect(await screen.findByText('No device is waiting to be added.')).toBeInTheDocument()
    focusManager.setFocused(false)
    const whileHidden = reads
    await new Promise((resolve) => setTimeout(resolve, 5000))
    expect(reads).toBe(whileHidden)
    const other = await otherDeviceAsks(node, 'Tablet')
    focusManager.setFocused(true)
    expect(await screen.findByLabelText('Pairing code of Tablet')).toHaveTextContent(other.pairingCode)
  }, 20_000)

  it('takes the notice on the other screens away by itself once the device is approved', async () => {
    const node = createMockNode()
    const keyStore = memoryKeyStore()
    renderApp({ path: '/overview', node, enrolled: false, keyStore })
    const key = await browserKey(keyStore)
    await createApiClient((request) => node.handle(request)).requestDevice({ label: 'Browser', publicKey: key.publicKey })
    expect(await screen.findByText('This device is waiting to be added. Open it to see the code to confirm.', undefined, { timeout: 9000 })).toBeInTheDocument()
    node.addDeviceFromTerminal(key.pairingCode)
    await waitFor(() => expect(screen.queryByText('This device is waiting to be added. Open it to see the code to confirm.')).not.toBeInTheDocument(), { timeout: 9000 })
  })
})

describe('the prompt to add this device', () => {
  it('leads from the first screen to adding it, in one sentence that says why', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/desk', enrolled: false })
    const notice = (await screen.findByText('Decisions and comments need a device of yours. Add this one: get its code, then confirm it on the node.')).closest('.device-notice')
    expect(notice).not.toBeNull()
    const [fromNotice] = screen.getAllByRole('link', { name: 'Add this device' })
    await user.click(fromNotice as HTMLElement)
    expect(await screen.findByRole('heading', { level: 1, name: 'Devices' })).toBeInTheDocument()
    expect(screen.queryByText('Decisions and comments need a device of yours. Add this one: get its code, then confirm it on the node.', { selector: '.device-notice p' })).not.toBeInTheDocument()
  })

  it('says the device is waiting once it asked, and points to its code', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    const keyStore = memoryKeyStore()
    const held = await browserKey(keyStore)
    await createApiClient((request) => node.handle(request)).requestDevice({ label: 'Browser', publicKey: held.publicKey })
    renderApp({ path: '/desk', node, keyStore })
    expect(await screen.findByText('This device is waiting to be added. Open it to see the code to confirm.')).toBeInTheDocument()
    await user.click(screen.getByRole('link', { name: 'See the code' }))
    expect(await screen.findByLabelText('Pairing code of this device')).toHaveTextContent(held.pairingCode)
  })

  it('is gone once the device is added', async () => {
    const node = createMockNode()
    const keyStore = memoryKeyStore()
    const held = await browserKey(keyStore)
    await createApiClient((request) => node.handle(request)).requestDevice({ label: 'Browser', publicKey: held.publicKey })
    node.addDeviceFromTerminal(held.pairingCode)
    renderApp({ path: '/desk', node, keyStore })
    await screen.findByRole('option', { name: /Add a second frontend agent/ })
    expect(document.querySelector('.device-notice')).toBeNull()
  })
})

describe('an empty desk', () => {
  it('says nothing waits, shows no blank paper to pick from, and has no one waiting on the viewer', async () => {
    renderApp({ path: '/desk', nothingWaiting: true, earlierDecisions: false })
    expect(await screen.findByText('Nothing is waiting for you. New requests from your team show up here.')).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Nothing needs your decision right now.')
    expect(screen.queryByRole('article')).not.toBeInTheDocument()
    expect(screen.queryByText(/Pick a document/)).not.toBeInTheDocument()
  })
})
