import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { MOCK_VAPID_KEY } from '../mock/webPush'
import { keyBytes } from '../pwa/webPush'
import { installFakeNotification, type FakeNotifications } from '../test/fakeNotification'
import { fakePushBrowser } from '../test/fakePushBrowser'
import { heldNode } from '../test/heldNode'
import { renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'

const TURN_ON = 'Turn on for a closed app'
const TURN_OFF = 'Turn off for a closed app'

describe('the switch for notifications with the app closed', () => {
  let fake: FakeNotifications
  beforeEach(() => {
    fake = installFakeNotification('default')
  })
  afterEach(() => fake.restore())

  it('is not drawn when the node has no Web Push', async () => {
    renderApp({ path: '/devices', enrolled: true, webPushBrowser: fakePushBrowser().browser })
    await screen.findByRole('button', { name: 'Turn on notifications' })
    await waitFor(() => expect(screen.queryByText('When the app is closed')).not.toBeInTheDocument())
  })

  it('is not drawn in a browser that has no Web Push, or on an iPhone, which has the app', async () => {
    for (const browser of [fakePushBrowser({ isOffered: false }).browser, fakePushBrowser({ userAgent: 'iphone' }).browser]) {
      const { unmount } = renderApp({ path: '/devices', node: createTestNode({ webPush: true }), enrolled: true, webPushBrowser: browser })
      await screen.findByRole('button', { name: 'Turn on notifications' })
      expect(screen.queryByText('When the app is closed')).not.toBeInTheDocument()
      unmount()
    }
  })

  it('asks for nothing before it is clicked, then subscribes with the node’s key and registers the browser at the node', async () => {
    const push = fakePushBrowser()
    const node = createTestNode({ webPush: true })
    const user = userEvent.setup()
    renderApp({ path: '/devices', node, enrolled: true, webPushBrowser: push.browser })
    const button = await screen.findByRole('button', { name: TURN_ON })
    expect(push.askedPermission()).toBe(0)
    expect(push.subscribed()).toEqual([])
    await user.click(button)
    expect(await screen.findByRole('button', { name: TURN_OFF })).toBeInTheDocument()
    expect(push.askedPermission()).toBe(1)
    expect(push.subscribed()).toEqual([{ userVisibleOnly: true, applicationServerKey: keyBytes(MOCK_VAPID_KEY.publicKey) }])
    expect(node.pushSubscriptions()).toHaveLength(1)
    expect(node.pushSubscriptions()[0]).toMatchObject({ label: 'Chrome on macOS', events: ['request'], keyId: MOCK_VAPID_KEY.keyId })
    expect(screen.getByText(/The node tells this browser when something is new, with the page closed/)).toBeInTheDocument()
  })

  it('does not ask again for a permission it has, and turns off at the node and at the browser', async () => {
    const push = fakePushBrowser({ permission: 'granted' })
    const node = createTestNode({ webPush: true })
    const user = userEvent.setup()
    renderApp({ path: '/devices', node, enrolled: true, webPushBrowser: push.browser })
    await user.click(await screen.findByRole('button', { name: TURN_ON }))
    await screen.findByRole('button', { name: TURN_OFF })
    expect(push.askedPermission()).toBe(0)
    await user.click(screen.getByRole('button', { name: TURN_OFF }))
    expect(await screen.findByRole('button', { name: TURN_ON })).toBeInTheDocument()
    expect(node.pushSubscriptions()).toEqual([])
    expect(push.unsubscribed()).toBe(1)
    expect(push.subscription()).toBeNull()
  })

  it('draws nothing of its own when the person refuses: the first switch says why', async () => {
    const push = fakePushBrowser()
    push.answer('denied')
    const user = userEvent.setup()
    renderApp({ path: '/devices', node: createTestNode({ webPush: true }), enrolled: true, webPushBrowser: push.browser })
    await user.click(await screen.findByRole('button', { name: TURN_ON }))
    await waitFor(() => expect(screen.queryByRole('button', { name: TURN_ON })).not.toBeInTheDocument())
    expect(push.subscribed()).toEqual([])
    expect(screen.queryByText('When the app is closed')).not.toBeInTheDocument()
  })

  it('is on already when this browser holds a subscription made with the node’s key, and tells the node again', async () => {
    const push = fakePushBrowser({ permission: 'granted' })
    push.holds()
    const node = createTestNode({ webPush: true })
    renderApp({ path: '/devices', node, enrolled: true, webPushBrowser: push.browser })
    expect(await screen.findByRole('button', { name: TURN_OFF })).toBeInTheDocument()
    await waitFor(() => expect(node.pushSubscriptions()).toHaveLength(1))
    expect(push.subscribed()).toEqual([])
  })

  it('says to subscribe again when the subscription was made with a key the node no longer has, and replaces it', async () => {
    const push = fakePushBrowser({ permission: 'granted' })
    push.holds(Uint8Array.from([4, 9, 9, 9]))
    const node = createTestNode({ webPush: true })
    const user = userEvent.setup()
    renderApp({ path: '/devices', node, enrolled: true, webPushBrowser: push.browser })
    expect(await screen.findByText(/subscribed with a key the node no longer has/)).toBeInTheDocument()
    expect(node.pushSubscriptions()).toEqual([])
    await user.click(screen.getByRole('button', { name: 'Subscribe again' }))
    expect(await screen.findByRole('button', { name: TURN_OFF })).toBeInTheDocument()
    expect(push.unsubscribed(), 'the old one goes first: a browser refuses a second key').toBe(1)
    expect(push.subscribed()).toEqual([{ userVisibleOnly: true, applicationServerKey: keyBytes(MOCK_VAPID_KEY.publicKey) }])
    expect(node.pushSubscriptions()).toHaveLength(1)
  })

  it('says what went wrong when the browser cannot subscribe, and can be tried again', async () => {
    const push = fakePushBrowser({ permission: 'granted' })
    push.failNextSubscribe('Registration failed - push service error')
    const user = userEvent.setup()
    renderApp({ path: '/devices', node: createTestNode({ webPush: true }), enrolled: true, webPushBrowser: push.browser })
    await user.click(await screen.findByRole('button', { name: TURN_ON }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Could not change it: Registration failed - push service error')
    await user.click(screen.getByRole('button', { name: TURN_ON }))
    expect(await screen.findByRole('button', { name: TURN_OFF })).toBeInTheDocument()
  })

  it('registers the renewed subscription when the worker says the browser renewed it', async () => {
    const push = fakePushBrowser({ permission: 'granted' })
    push.holds()
    const node = createTestNode({ webPush: true })
    renderApp({ path: '/devices', node, enrolled: true, webPushBrowser: push.browser })
    await screen.findByRole('button', { name: TURN_OFF })
    await waitFor(() => expect(node.pushSubscriptions()).toHaveLength(1))
    push.holds()
    push.renewed()
    await waitFor(() => expect(node.pushSubscriptions()).toHaveLength(2), { timeout: 3_000 })
  })

  it('shows a placeholder, not "off", until the node has given its key', async () => {
    const node = createTestNode({ webPush: true })
    const { client, release } = heldNode(node, ['/api/push/vapid'])
    renderApp({ path: '/devices', node, client, enrolled: true, webPushBrowser: fakePushBrowser().browser })
    await screen.findByRole('button', { name: 'Turn on notifications' })
    await waitFor(() => expect(document.querySelector('.push-switch .skeleton')).not.toBeNull())
    expect(screen.queryByRole('button', { name: TURN_ON })).not.toBeInTheDocument()
    release()
    expect(await screen.findByRole('button', { name: TURN_ON })).toBeInTheDocument()
  })

  it('reads in Japanese', async () => {
    renderApp({ path: '/devices', node: createTestNode({ webPush: true }), enrolled: true, locale: 'ja', webPushBrowser: fakePushBrowser().browser })
    expect(await screen.findByRole('button', { name: '閉じているときも通知する' })).toBeInTheDocument()
    expect(screen.getByText('アプリを閉じているとき')).toBeInTheDocument()
  })

  it('asks for the permission at once inside the click, before it has read the node’s key again', async () => {
    const push = fakePushBrowser()
    const node = createTestNode({ webPush: true })
    let keyReads = 0
    // The first read of the key (when the screen opens) answers; the next one, which the click makes, never does.
    const client = createApiClient((request) => {
      const isKey = new URL(request.url, 'http://mock.local').pathname === '/api/push/vapid'
      if (isKey && (keyReads += 1) > 1) return new Promise<Response>(() => undefined)
      return node.handle(request)
    })
    const user = userEvent.setup()
    renderApp({ path: '/devices', node, client, enrolled: true, webPushBrowser: push.browser })
    await user.click(await screen.findByRole('button', { name: TURN_ON }))
    expect(push.askedPermission(), 'asked while the key is still being read').toBe(1)
    expect(keyReads).toBe(2)
    expect(push.subscribed()).toEqual([])
  })
})

describe('a subscription the browser holds, wherever the viewer is', () => {
  let fake: FakeNotifications
  beforeEach(() => {
    fake = installFakeNotification('granted')
  })
  afterEach(() => fake.restore())

  it('is registered at the node by any page of the app, not only the Devices screen', async () => {
    const push = fakePushBrowser({ permission: 'granted' })
    push.holds()
    const node = createTestNode({ webPush: true })
    renderApp({ path: '/overview', node, enrolled: true, webPushBrowser: push.browser })
    await waitFor(() => expect(node.pushSubscriptions()).toHaveLength(1))
  })

  it('is registered again when the worker says the browser renewed it, with the app open anywhere', async () => {
    const push = fakePushBrowser({ permission: 'granted' })
    push.holds()
    const node = createTestNode({ webPush: true })
    renderApp({ path: '/overview', node, enrolled: true, webPushBrowser: push.browser })
    await waitFor(() => expect(node.pushSubscriptions()).toHaveLength(1))
    push.holds()
    push.renewed()
    await waitFor(() => expect(node.pushSubscriptions()).toHaveLength(2))
  })

  it('asks the node for nothing when the browser holds no subscription or has no permission', async () => {
    const paths: string[] = []
    const node = createTestNode({ webPush: true })
    const client = createApiClient((request) => (paths.push(new URL(request.url, 'http://mock.local').pathname), node.handle(request)))
    renderApp({ path: '/overview', node, client, enrolled: true, webPushBrowser: fakePushBrowser({ permission: 'granted' }).browser })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    const refused = fakePushBrowser({ permission: 'denied' })
    refused.holds()
    renderApp({ path: '/overview', node, client, enrolled: true, webPushBrowser: refused.browser })
    await screen.findAllByRole('heading', { level: 1, name: 'Overview' })
    expect(paths.filter((path) => path.startsWith('/api/push'))).toEqual([])
    expect(node.pushSubscriptions()).toEqual([])
  })

  it('does not ask the viewer for anything to register it', async () => {
    const push = fakePushBrowser({ permission: 'granted' })
    push.holds()
    const node = createTestNode({ webPush: true })
    renderApp({ path: '/overview', node, enrolled: true, webPushBrowser: push.browser })
    await waitFor(() => expect(node.pushSubscriptions()).toHaveLength(1))
    expect([push.askedPermission(), push.subscribed()]).toEqual([0, []])
  })
})
