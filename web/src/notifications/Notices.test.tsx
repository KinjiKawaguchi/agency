import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { createMockNode } from '../mock/node'
import { installFakeNotification, setPageVisibility, type FakeNotifications } from '../test/fakeNotification'
import { DEVICES_KEY, TEAM_KEY } from '../api/queries'
import { currentQueryClient, refreshQueries, renderApp } from '../test/render'
import { createTestNode, decideAsTestDevice } from '../test/testDevice'
import { deviceKeyOf, generateKeyPair } from '../device/deviceKey'

const askFrom = async (node: ReturnType<typeof createMockNode>, label: string) => {
  const key = await deviceKeyOf(await generateKeyPair())
  await createApiClient((request) => node.handle(request)).requestDevice({ label, publicKey: key.publicKey })
}

describe('what is new, said in the page', () => {
  it('tells the viewer a device asks to be added, once, and marks the avatar with how many wait', async () => {
    const node = createTestNode()
    renderApp({ path: '/overview', node, enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await new Promise((resolve) => setTimeout(resolve, 100))
    expect(screen.queryByText(/asks to be added/)).not.toBeInTheDocument()
    await askFrom(node, 'Phone')
    window.dispatchEvent(new Event('visibilitychange'))
    expect(await screen.findByText('Phone asks to be added as a device.')).toBeInTheDocument()
    expect(screen.getAllByText('Phone asks to be added as a device.')).toHaveLength(1)
    const avatar = screen.getByRole('link', { name: 'Devices, 1 waiting to be approved' })
    expect(within(avatar).getByText('1')).toBeInTheDocument()
  })

  it('says nothing about a device on the devices screen, whose list shows it by itself', async () => {
    const node = createTestNode()
    renderApp({ path: '/devices', node, enrolled: true })
    await screen.findByText('No device is waiting to be added.')
    await askFrom(node, 'Tablet')
    window.dispatchEvent(new Event('visibilitychange'))
    expect(await screen.findByLabelText('Pairing code of Tablet')).toBeInTheDocument()
    expect(screen.queryByText('Tablet asks to be added as a device.')).not.toBeInTheDocument()
  })

  it('does not mark the avatar when this device is not added, since it can approve nothing', async () => {
    const node = createMockNode()
    await askFrom(node, 'Phone')
    renderApp({ path: '/overview', node, enrolled: false })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await new Promise((resolve) => setTimeout(resolve, 200))
    expect(screen.getByRole('link', { name: 'Devices' })).toBeInTheDocument()
    expect(document.querySelector('.me-count')).toBeNull()
  })

  it('tells the viewer a decision made on another device went through', async () => {
    const node = createTestNode()
    renderApp({ path: '/overview', node, enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await new Promise((resolve) => setTimeout(resolve, 200))
    const api = createApiClient((request) => node.handle(request))
    await decideAsTestDevice(api, { org: 'acme', team: 'app', answerWith: { kind: 'answer', target: 'r-20261124-113000-c3d4' }, title: 'Add a second frontend agent', choice: 'approve', text: '' })
    window.dispatchEvent(new Event('visibilitychange'))
    expect(await screen.findByText('A decision “Add a second frontend agent” was applied.')).toBeInTheDocument()
  })

  it('does not tell the viewer that a decision they sent went through, and tells them when one they sent is refused', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    renderApp({ path: '/desk', node, enrolled: true })
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.click(screen.getByRole('button', { name: 'Approve with “Approve”' }))
    expect(await screen.findByText(/Sent\. Lead will get it\./)).toBeInTheDocument()
    window.dispatchEvent(new Event('visibilitychange'))
    await new Promise((resolve) => setTimeout(resolve, 300))
    expect(screen.queryByText(/was applied/)).not.toBeInTheDocument()
  })
})

describe('what is new, said by the operating system while the page is hidden', () => {
  let fake: FakeNotifications
  beforeEach(() => {
    fake = installFakeNotification('granted')
    window.localStorage.setItem('agency.osNotices', 'on')
    setPageVisibility('hidden')
  })
  afterEach(() => {
    fake.restore()
    setPageVisibility('visible')
  })

  const settle = () => new Promise((resolve) => setTimeout(resolve, 150))

  it('shows a notification in the words of the page’s notice, and leads to what it says when it is clicked', async () => {
    const node = createTestNode()
    const { history } = renderApp({ path: '/overview', node, enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await settle()
    expect(fake.shown).toHaveLength(0)
    await askFrom(node, 'Phone')
    await refreshQueries()
    expect(await screen.findByText('Phone asks to be added as a device.')).toBeInTheDocument()
    expect(fake.shown.map((one) => [one.title, one.options?.body])).toEqual([['Agency', 'Phone asks to be added as a device.']])
    const focus = vi.spyOn(window, 'focus').mockImplementation(() => undefined)
    fake.shown[0]?.onclick?.()
    expect(focus).toHaveBeenCalled()
    expect(fake.shown[0]?.closed).toBe(true)
    expect(await screen.findByRole('heading', { level: 1, name: 'Devices' })).toBeInTheDocument()
    expect(history.location.pathname).toBe('/devices')
  })

  it('leads a decision to the desk', async () => {
    const node = createTestNode()
    const { history } = renderApp({ path: '/overview', node, enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await settle()
    await decideAsTestDevice(createApiClient((request) => node.handle(request)), { org: 'acme', team: 'app', answerWith: { kind: 'answer', target: 'r-20261124-113000-c3d4' }, title: 'Add a second frontend agent', choice: 'approve', text: '' })
    await refreshQueries()
    await waitFor(() => expect(fake.shown).toHaveLength(1))
    expect(fake.shown[0]?.options?.body).toBe('A decision “Add a second frontend agent” was applied.')
    vi.spyOn(window, 'focus').mockImplementation(() => undefined)
    fake.shown[0]?.onclick?.()
    await waitFor(() => expect(history.location.pathname).toBe('/desk'))
  })

  it('says no title to the operating system when the node hides titles, though the page’s own notice keeps it', async () => {
    const node = createTestNode({ hidesTitles: true })
    renderApp({ path: '/overview', node, enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await settle()
    await askFrom(node, 'Phone')
    await refreshQueries()
    expect(await screen.findByText('Phone asks to be added as a device.')).toBeInTheDocument()
    await waitFor(() => expect(fake.shown).toHaveLength(1))
    expect(fake.shown[0]?.options?.body).toBe('A device is waiting to be added')
  })

  it('says no title for a decision either, when the node hides titles', async () => {
    const node = createTestNode({ hidesTitles: true })
    renderApp({ path: '/overview', node, enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await settle()
    await decideAsTestDevice(createApiClient((request) => node.handle(request)), { org: 'acme', team: 'app', answerWith: { kind: 'answer', target: 'r-20261124-113000-c3d4' }, title: 'Add a second frontend agent', choice: 'approve', text: '' })
    await refreshQueries()
    expect(await screen.findByText('A decision “Add a second frontend agent” was applied.')).toBeInTheDocument()
    await waitFor(() => expect(fake.shown).toHaveLength(1))
    expect(fake.shown[0]?.options?.body).toBe('A decision was applied.')
  })

  it('says the title when the node says it does not hide titles', async () => {
    const node = createTestNode({ hidesTitles: false })
    renderApp({ path: '/overview', node, enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await settle()
    await askFrom(node, 'Phone')
    await refreshQueries()
    await waitFor(() => expect(fake.shown).toHaveLength(1))
    expect(fake.shown[0]?.options?.body).toBe('Phone asks to be added as a device.')
  })

  it('says no title until the node has answered who the viewer is and whether it hides titles', async () => {
    const node = createTestNode({ hidesTitles: false })
    // The account is never answered; the device list and the decision log are, and they carry labels and titles.
    const client = createApiClient(async (request) => {
      if (request.method === 'GET' && new URL(request.url, 'http://mock.local').pathname === '/api/me') await new Promise<never>(() => undefined)
      return node.handle(request)
    })
    renderApp({ path: '/overview', node, client, enrolled: true })
    await settle()
    await askFrom(node, 'Phone')
    // Only the device list: the account's read never ends, and waiting for it would wait for ever.
    await refreshQueries(DEVICES_KEY)
    await waitFor(() => expect(fake.shown).toHaveLength(1))
    expect(fake.shown[0]?.options?.body).toBe('A device is waiting to be added')
  })

  it('shows none while the page is in front, though the page’s own notice appears', async () => {
    setPageVisibility('visible')
    const node = createTestNode()
    renderApp({ path: '/overview', node, enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await settle()
    await askFrom(node, 'Phone')
    await refreshQueries()
    expect(await screen.findByText('Phone asks to be added as a device.')).toBeInTheDocument()
    expect(fake.shown).toHaveLength(0)
  })

  it('shows none unless the viewer turned it on', async () => {
    window.localStorage.removeItem('agency.osNotices')
    const node = createTestNode()
    renderApp({ path: '/overview', node, enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await settle()
    await askFrom(node, 'Phone')
    await refreshQueries()
    expect(await screen.findByText('Phone asks to be added as a device.')).toBeInTheDocument()
    expect(fake.shown).toHaveLength(0)
  })

  const deviceReadsWhileHidden = async (): Promise<number> => {
    const node = createTestNode()
    let reads = 0
    const counted = createApiClient((request) => {
      if (request.method === 'GET' && new URL(request.url, 'http://mock.local').pathname === '/api/devices') reads += 1
      return node.handle(request)
    })
    renderApp({ path: '/overview', node, enrolled: true, client: counted, devicesPollMs: 50 })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await settle()
    const before = reads
    // Twenty polls would come in this time.
    await new Promise((resolve) => setTimeout(resolve, 1000))
    return reads - before
  }

  it('keeps reading while the page is hidden, to have something to show', async () => {
    expect(await deviceReadsWhileHidden()).toBeGreaterThan(3)
  })

  it('does not read while the page is hidden when the viewer did not turn it on', async () => {
    window.localStorage.removeItem('agency.osNotices')
    expect(await deviceReadsWhileHidden()).toBe(0)
  })

  /** Whether some part of the page asks for each team's read to go on while the tab is hidden. */
  const teamsReadInBackground = (): boolean[] =>
    currentQueryClient()
      .getQueryCache()
      .findAll({ queryKey: TEAM_KEY })
      .map((query) => query.observers.some((observer) => observer.options.refetchIntervalInBackground === true))

  it('asks for each team to be read in the background too, when it is on, and not when it is off', async () => {
    renderApp({ path: '/overview', enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await settle()
    expect(teamsReadInBackground().length).toBeGreaterThan(0)
    expect(teamsReadInBackground().every(Boolean)).toBe(true)
  })

  it('does not ask for it when it is off', async () => {
    window.localStorage.removeItem('agency.osNotices')
    renderApp({ path: '/overview', enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await settle()
    expect(teamsReadInBackground().length).toBeGreaterThan(0)
    expect(teamsReadInBackground().some(Boolean)).toBe(false)
  })

  it('shows none once the browser no longer allows them', async () => {
    const node = createTestNode()
    renderApp({ path: '/overview', node, enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Overview' })
    await settle()
    fake.setPermission('denied')
    await askFrom(node, 'Phone')
    await refreshQueries()
    expect(await screen.findByText('Phone asks to be added as a device.')).toBeInTheDocument()
    expect(fake.shown).toHaveLength(0)
  })
})
