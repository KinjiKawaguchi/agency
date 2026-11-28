import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { sampleTeamView } from '../api/teamview'
import { createMockNode } from '../mock/node'
import { heldNode } from '../test/heldNode'
import { currentQueryClient, memoryKeyStore, refreshQueries, renderApp } from '../test/render'
import { settledDeviceKey } from '../device/useDeviceKey'
import { guestOf } from './provisional'
import { isGuestPage } from './useStanding'

const GUEST = { member: 'carol', label: 'Carol' }
const PROPOSAL = 'Add a second frontend agent'
const BOBS = 'Which sample should the next run use?'

/** A title or a phrase may be on the page more than once (a card and its list). */
const shown = (text: string | RegExp) => screen.findAllByText(text)

const labels = (menu: HTMLElement): (string | null)[] => within(menu).getAllByRole('link').map((link) => link.textContent?.replace(/\d+$/, '') ?? null)

/** A node whose team also has a request addressed to someone else, which a guest must not be given. */
function guestNode() {
  const first = sampleTeamView.requests[0]
  if (first === undefined) throw new Error('the sample has no request')
  const view = { ...sampleTeamView, requests: [...sampleTeamView.requests, { ...first, id: 'r-for-bob', title: BOBS, to: 'bob' }] }
  return createMockNode({ guest: GUEST, view })
}

async function enrol(node: ReturnType<typeof createMockNode>, keyStore: ReturnType<typeof memoryKeyStore>): Promise<string> {
  const held = await settledDeviceKey(keyStore)
  if (held.status !== 'ready') throw new Error('no key')
  await createApiClient((request) => node.handle(request)).requestDevice({ label: 'Browser', publicKey: held.key.publicKey })
  node.addDeviceFromTerminal(held.key.pairingCode)
  return `d-${held.key.pairingCode.replaceAll('-', '')}`
}

/** What the owner alone is given: the org's definition and its reads, token use, the activity, and push settings. */
const OWNERS = /\/api\/orgs\/[^/]+(\/(evaluation|effort|history|outcomes(\/readings)?)|\/teams\/[^/]+\/(usage|activity))?$|\/api\/push\//

/** A client over the node that notes the path of every call it makes. */
function recording(node: ReturnType<typeof createMockNode>): { client: ReturnType<typeof createApiClient>; paths: string[] } {
  const paths: string[] = []
  const client = createApiClient((request) => {
    paths.push(new URL(request.url, 'http://mock.local').pathname)
    return node.handle(request)
  })
  return { client, paths }
}

describe('what a member hosted by the node has', () => {
  it('has two entries, their decisions and their devices, and none of the owner’s pages', async () => {
    renderApp({ node: guestNode(), path: '/desk', enrolled: true })
    const main = await screen.findByRole('navigation', { name: 'Main menu' })
    await shown(PROPOSAL)
    expect(labels(main)).toEqual(['Decisions', 'Devices'])
    expect(within(main).getByRole('link', { name: /^Decisions/ })).toHaveAttribute('aria-current', 'page')
    expect(screen.queryByRole('navigation', { name: /pages$/ })).not.toBeInTheDocument()
  })

  it('is shown only the requests addressed to them', async () => {
    renderApp({ node: guestNode(), path: '/desk', enrolled: true })
    expect(await shown(PROPOSAL)).not.toHaveLength(0)
    expect(await shown('Which database should the app use in production?')).not.toHaveLength(0)
    expect(screen.queryByText(BOBS)).not.toBeInTheDocument()
  })

  it.each(['/', '/overview', '/org', '/history', '/people', '/costs', '/evaluation', '/live', '/outcomes', '/nowhere'])('is sent from %s to their decisions, and none of the owner’s reads is made', async (path) => {
    const { client, paths } = recording(guestNode())
    const { history } = renderApp({ node: guestNode(), client, path, enrolled: true })
    await waitFor(() => expect(history.location.pathname).toBe('/desk'))
    await shown(PROPOSAL)
    expect(paths.filter((one) => OWNERS.test(one))).toEqual([])
  })

  it('leaves the claim page as it is, with none of the app’s pages and none of the owner’s reads', async () => {
    const { client, paths } = recording(guestNode())
    const { history } = renderApp({ node: guestNode(), client, path: '/claim', enrolled: true })
    expect(await screen.findByText(/link/i)).toBeInTheDocument()
    expect(history.location.pathname).toBe('/claim')
    expect(screen.queryByRole('navigation', { name: 'Main menu' })).not.toBeInTheDocument()
    expect(paths.filter((one) => OWNERS.test(one))).toEqual([])
  })

  it.each(['/desk/x', '/devices/x'])('stays on %s, a page that is not there, and makes none of the owner’s reads', async (path) => {
    const { client, paths } = recording(guestNode())
    const { history } = renderApp({ node: guestNode(), client, path, enrolled: true })
    expect(await screen.findByText(/not found/i)).toBeInTheDocument()
    expect(history.location.pathname).toBe(path)
    expect(labels(await screen.findByRole('navigation', { name: 'Main menu' }))).toEqual(['Decisions', 'Devices'])
    expect(paths.filter((one) => OWNERS.test(one))).toEqual([])
  })

  it.each(['/desk', '/devices'])('draws no area of funding on %s: the node says none for a guest, and none is shown', async (path) => {
    renderApp({ node: guestNode(), path, enrolled: true })
    await screen.findByRole('navigation', { name: 'Main menu' })
    await waitFor(() => expect(screen.queryByRole('heading', { name: 'Loading…' })).not.toBeInTheDocument())
    expect(screen.queryByText(/Usage limits|Subscriptions/)).not.toBeInTheDocument()
  })

  it('keeps them on their devices, and leaves out what is the owner’s there: the other nodes of the owner', async () => {
    const { history } = renderApp({ node: guestNode(), path: '/devices', enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Devices' })
    await shown(/This device/)
    expect(history.location.pathname).toBe('/devices')
    expect(screen.queryByRole('heading', { name: /Home nodes/ })).not.toBeInTheDocument()
    const main = screen.getByRole('navigation', { name: 'Main menu' })
    expect(within(main).getByRole('link', { name: /^Devices/ })).toHaveAttribute('aria-current', 'page')
  })

  it('answers a request with their own device, as their member', async () => {
    const user = userEvent.setup()
    const node = guestNode()
    const keyStore = memoryKeyStore()
    const deviceId = await enrol(node, keyStore)
    renderApp({ node, keyStore })
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.type(screen.getByRole('textbox', { name: /^Notes/ }), 'Go ahead.')
    await user.click(screen.getByRole('button', { name: 'Approve with “Approve”' }))
    await waitFor(() => expect(node.envelopes()).toHaveLength(1))
    expect(node.envelopes()).toMatchObject([{ signer: deviceId, body: { kind: 'answer', org: 'acme', team: 'app', account: 'carol' } }])
  })

  it('says nothing of the owner’s numbers: no funding, and no node command for adding a device', async () => {
    const node = guestNode()
    const api = createApiClient((request) => node.handle(request))
    const me = await api.me()
    expect(me).toMatchObject({ account: { id: 'carol' }, funding: [], guest: { member: 'carol', org: 'acme' } })
    expect(me.orgs.flatMap((org) => org.teams.map((team) => team.id))).toEqual(['app'])
    expect((await api.devices()).node).toBeUndefined()
  })
})

describe('before the node has said who the viewer is', () => {
  it('shows no entry and opens no page, so that a guest is not offered the owner’s pages for a moment', async () => {
    const node = guestNode()
    const { client, release } = heldNode(node, ['/api/me'])
    const { history } = renderApp({ node, client, path: '/overview', enrolled: true })
    await screen.findByRole('navigation', { name: 'Main menu' })
    expect(within(screen.getByRole('navigation', { name: 'Main menu' })).queryAllByRole('link')).toHaveLength(0)
    expect(screen.queryByRole('heading', { level: 1, name: 'Overview' })).not.toBeInTheDocument()
    expect(history.location.pathname).toBe('/overview')
    release()
    await waitFor(() => expect(history.location.pathname).toBe('/desk'))
    expect(labels(await screen.findByRole('navigation', { name: 'Main menu' }))).toEqual(['Decisions', 'Devices'])
  })

  it('shows no home nodes of the owner before the node has said who the viewer is, and shows them to the owner once it has', async () => {
    const node = createMockNode({ homeNodes: [{ label: 'Work', url: 'https://work.example.com' }] })
    const { client, release } = heldNode(node, ['/api/me'])
    renderApp({ node, client, path: '/devices', enrolled: true })
    await screen.findByRole('heading', { level: 1, name: 'Devices' })
    expect(screen.queryByRole('region', { name: 'Home nodes' })).not.toBeInTheDocument()
    release()
    expect(await screen.findByRole('region', { name: 'Home nodes' })).toBeInTheDocument()
  })

  it('gives everyone else their pages as before, once it has', async () => {
    const node = createMockNode()
    const { client, release } = heldNode(node, ['/api/me'])
    const { history } = renderApp({ node, client, path: '/overview', enrolled: true })
    release()
    expect(await screen.findByRole('heading', { level: 1, name: 'Overview' })).toBeInTheDocument()
    expect(history.location.pathname).toBe('/overview')
    expect(labels(screen.getByRole('navigation', { name: 'Main menu' }))).toEqual(['Decisions', 'Overview', 'Organization', 'Results'])
  })

  it('treats a first read that failed as no guest, and keeps the owner’s menu, so the pages show their own failures', async () => {
    const node = createMockNode()
    const client = createApiClient((request) => (new URL(request.url, 'http://mock.local').pathname === '/api/me' ? Promise.resolve(new Response(JSON.stringify({ error: 'down' }), { status: 500, headers: { 'content-type': 'application/json' } })) : node.handle(request)))
    const { history } = renderApp({ node, client, path: '/overview', enrolled: true })
    await waitFor(() => expect(labels(screen.getByRole('navigation', { name: 'Main menu' }))).toEqual(['Decisions', 'Overview', 'Organization', 'Results']))
    expect(history.location.pathname).toBe('/overview')
  })

  it('opens the owner’s menu for a guest whose first read of the viewer failed, as for anyone: the node refuses each read, so nothing of the owner’s is shown', async () => {
    const node = guestNode()
    const client = createApiClient((request) => (new URL(request.url, 'http://mock.local').pathname === '/api/me' ? Promise.resolve(new Response(JSON.stringify({ error: 'down' }), { status: 500, headers: { 'content-type': 'application/json' } })) : node.handle(request)))
    const { history } = renderApp({ node, client, path: '/overview', enrolled: true })
    await waitFor(() => expect(labels(screen.getByRole('navigation', { name: 'Main menu' }))).toEqual(['Decisions', 'Overview', 'Organization', 'Results']))
    expect(history.location.pathname).toBe('/overview')
    expect(screen.queryByText(BOBS)).not.toBeInTheDocument()
    expect(screen.queryByText(PROPOSAL)).not.toBeInTheDocument()
  })

  it('keeps an owner’s page open while a read of the viewer is tried again after it failed', async () => {
    const node = createMockNode()
    let isDown = true
    let release: () => void = () => undefined
    const held = new Promise<void>((resolve) => (release = resolve))
    const client = createApiClient(async (request) => {
      if (new URL(request.url, 'http://mock.local').pathname !== '/api/me') return node.handle(request)
      if (isDown) return new Response(JSON.stringify({ error: 'down' }), { status: 500, headers: { 'content-type': 'application/json' } })
      await held
      return node.handle(request)
    })
    renderApp({ node, client, path: '/overview', enrolled: true })
    expect(await screen.findByRole('heading', { level: 1, name: 'Overview' })).toBeInTheDocument()
    isDown = false
    // A read started again after a failure shows as pending with no error, but the page is not taken away for it.
    void currentQueryClient().refetchQueries({ queryKey: ['me'] })
    await waitFor(() => expect(currentQueryClient().isFetching({ queryKey: ['me'] })).toBe(1))
    expect(screen.getByRole('heading', { level: 1, name: 'Overview' })).toBeInTheDocument()
    release()
    await waitFor(() => expect(labels(screen.getByRole('navigation', { name: 'Main menu' }))).toEqual(['Decisions', 'Overview', 'Organization', 'Results']))
  })

  it('stays a guest when a later read of the answer fails: the answer held stands', async () => {
    const node = guestNode()
    let isDown = false
    const client = createApiClient((request) => (isDown && new URL(request.url, 'http://mock.local').pathname === '/api/me' ? Promise.resolve(new Response(JSON.stringify({ error: 'down' }), { status: 500, headers: { 'content-type': 'application/json' } })) : node.handle(request)))
    const { history } = renderApp({ node, client, path: '/desk', enrolled: true })
    await shown(PROPOSAL)
    isDown = true
    await refreshQueries(['me'])
    history.push('/overview')
    await waitFor(() => expect(history.location.pathname).toBe('/desk'))
    expect(labels(screen.getByRole('navigation', { name: 'Main menu' }))).toEqual(['Decisions', 'Devices'])
  })
})

describe('how the node says a viewer is a guest', () => {
  const base = createMockNode().handle(new Request('http://mock.local/api/me'))

  it('reads a member id when there is one, and counts any guest that is not null as a guest, in whatever shape', async () => {
    const me = await (await base).json()
    expect(guestOf(me)).toBeUndefined()
    expect(guestOf({ ...me, guest: null })).toBeUndefined()
    expect(guestOf({ ...me, guest: { member: 'carol', org: 'acme' } })).toEqual({ member: 'carol', org: 'acme' })
    expect(guestOf({ ...me, guest: { member: 'carol' } })).toEqual({ member: 'carol' })
    expect(guestOf({ ...me, guest: { member: 'carol', org: 7 } }), 'an org that is not text is not read').toEqual({ member: 'carol' })
    for (const guest of ['carol', 7, true, {}, { member: '' }, { member: 3 }, { member: null }, [], { id: 'carol' }, { org: '' }]) expect(guestOf({ ...me, guest }), JSON.stringify(guest)).toEqual({})
  })

  it('opens a guest’s pages, not the owner’s, when the node says the guest in another shape', async () => {
    const node = createMockNode()
    const client = createApiClient(async (request) => {
      const reply = await node.handle(request)
      return new URL(request.url, 'http://mock.local').pathname === '/api/me' ? new Response(JSON.stringify({ ...(await reply.json()), guest: { seat: 'carol' } }), { headers: { 'content-type': 'application/json' } }) : reply
    })
    const { history } = renderApp({ node, client, path: '/overview', enrolled: true })
    await waitFor(() => expect(history.location.pathname).toBe('/desk'))
    expect(labels(await screen.findByRole('navigation', { name: 'Main menu' }))).toEqual(['Decisions', 'Devices'])
  })

  it('has the desk and the devices as the pages of a guest, with what is under them, and no other', () => {
    expect(['/desk', '/desk/', '/devices', '/devices/x'].map(isGuestPage)).toEqual([true, true, true, true])
    expect(['/', '/overview', '/org', '/history', '/people', '/costs', '/evaluation', '/live', '/outcomes', '/deskx', '/devicesx', '/x/desk'].map(isGuestPage)).toEqual(Array(12).fill(false))
  })
})
