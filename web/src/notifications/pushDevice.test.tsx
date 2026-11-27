import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { installFakeNotification, type FakeNotifications } from '../test/fakeNotification'
import { fakePushBrowser } from '../test/fakePushBrowser'
import { refreshQueries, renderApp, memoryKeyStore } from '../test/render'
import { createTestNode, TEST_DEVICE_ID } from '../test/testDevice'
import { createMockNode } from '../mock/node'
import { settledDeviceKey } from '../device/useDeviceKey'

const TURN_ON = 'Turn on for a closed app'

describe('a push subscription names the device of this browser', () => {
  let fake: FakeNotifications
  beforeEach(() => {
    fake = installFakeNotification('default')
  })
  afterEach(() => fake.restore())

  it('names it when the node ties registrations to devices, so that removing the device stops the notices', async () => {
    const push = fakePushBrowser({ permission: 'granted' })
    const node = createTestNode({ webPush: true, pushDevice: true })
    const user = userEvent.setup()
    renderApp({ path: '/devices', node, enrolled: true, webPushBrowser: push.browser })
    await user.click(await screen.findByRole('button', { name: TURN_ON }))
    await screen.findByRole('button', { name: 'Turn off for a closed app' })
    expect(node.pushSubscriptions()).toHaveLength(1)
    expect(node.pushSubscriptions()[0]).toMatchObject({ device: TEST_DEVICE_ID })
  })

  it('does not name it to a node that does not tie them: an older node refuses a field it does not know', async () => {
    const push = fakePushBrowser({ permission: 'granted' })
    const node = createTestNode({ webPush: true })
    const user = userEvent.setup()
    renderApp({ path: '/devices', node, enrolled: true, webPushBrowser: push.browser })
    await user.click(await screen.findByRole('button', { name: TURN_ON }))
    await screen.findByRole('button', { name: 'Turn off for a closed app' })
    expect(node.pushSubscriptions()).toHaveLength(1)
    expect(node.pushSubscriptions()[0]).not.toHaveProperty('device')
  })

  it('registers without a device while this browser is not added, and names it once it is', async () => {
    const push = fakePushBrowser({ permission: 'granted' })
    const node = createMockNode({ webPush: true, pushDevice: true })
    const keyStore = memoryKeyStore()
    const user = userEvent.setup()
    renderApp({ path: '/devices', node, keyStore, webPushBrowser: push.browser })
    await user.click(await screen.findByRole('button', { name: TURN_ON }))
    await screen.findByRole('button', { name: 'Turn off for a closed app' })
    expect(node.pushSubscriptions()[0]).not.toHaveProperty('device')
    const held = await settledDeviceKey(keyStore)
    if (held.status !== 'ready') throw new Error('no key')
    await user.click(await screen.findByRole('button', { name: 'Add this device' }))
    await screen.findByText(/Waiting for you to approve this device/)
    expect(node.addDeviceFromTerminal(held.key.pairingCode)).toBe(true)
    await refreshQueries()
    await waitFor(() => expect(node.pushSubscriptions()[0]).toMatchObject({ device: `d-${held.key.pairingCode.replaceAll('-', '')}` }))
  })
})
