import { screen, waitFor, within } from '@testing-library/react'
import { createApiClient } from '../api/client'
import { renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'

const membersTab = () => screen.queryByRole('link', { name: 'Members' })
const section = async (name: string) => within((await screen.findByRole('heading', { level: 2, name })).closest('section') as HTMLElement)
/** A node whose members answer is `answer`, whatever the sample says. */
const answering = (answer: unknown) => {
  const node = createTestNode({ members: true })
  return { node, client: createApiClient(async (request) => (new URL(request.url, 'http://mock.local').pathname.endsWith('/members') ? new Response(JSON.stringify(answer), { headers: { 'content-type': 'application/json' } }) : node.handle(request))) }
}
const refused = (status: number) => (): Promise<Response> => Promise.resolve(new Response(JSON.stringify({ error: 'no' }), { status, headers: { 'content-type': 'application/json' } }))

describe('the owner’s members page', () => {
  it('draws the whole list the node gives: members with their devices, open seats, external parties, and what waits for the root key', async () => {
    renderApp({ path: '/members', node: createTestNode({ members: true }), enrolled: true })
    expect(await screen.findByRole('heading', { level: 1, name: 'Members' })).toBeInTheDocument()
    const members = await section('Members')
    expect(members.getByText('ALICE')).toBeInTheDocument()
    // Alice's three devices, with the kind of key each holds (a browser's key is P-256).
    expect(members.getAllByText(/^Laptop, phone or app key, added /).length).toBeGreaterThan(0)
    expect(members.getByText(/^iPad, browser key, added /)).toBeInTheDocument()
    expect(members.getAllByText('No device yet.')).toHaveLength(1)
    const seats = await section('Open seats')
    expect(seats.getByText('Ask about:', { exact: false })).toBeInTheDocument()
    const externals = await section('External parties')
    expect(externals.getByText('Erin (outside counsel)')).toBeInTheDocument()
    expect(externals.getByText('Answered by ALICE')).toBeInTheDocument()
    const held = await section('Waiting for the root key')
    expect(held.getByText(/^remove device d-2ad2694ace04 of carol, received /)).toBeInTheDocument()
  })

  it('says a hosted member is hosted by this node, with whether a sign-in is set, and never an address', async () => {
    const { container } = renderApp({ path: '/members', node: createTestNode({ members: true }), enrolled: true })
    const members = await section('Members')
    expect(members.getByText('Hosted by this node · A sign-in is set', { exact: false })).toBeInTheDocument()
    expect(container.textContent).not.toMatch(/@/)
  })

  it('is in the menu, under the organization, for the owner', async () => {
    renderApp({ path: '/members', node: createTestNode({ members: true }), enrolled: true })
    await section('Members')
    await waitFor(() => expect(membersTab()).toBeInTheDocument())
    expect(membersTab()).toHaveAttribute('href', expect.stringContaining('/members'))
  })

  it('does not ask a node that does not list the capability, says it does not report members, and has no entry in the menu', async () => {
    const node = createTestNode()
    const asked: string[] = []
    const client = createApiClient((request) => {
      asked.push(new URL(request.url, 'http://mock.local').pathname)
      return node.handle(request)
    })
    renderApp({ path: '/members', node, client, enrolled: true })
    expect(await screen.findByText('This node does not report members yet.')).toBeInTheDocument()
    expect(asked.some((path) => path.endsWith('/members'))).toBe(false)
    expect(membersTab()).not.toBeInTheDocument()
  })

  it('shows nothing, says nothing and has no menu entry for anyone the node answers 403 (not the owner)', async () => {
    const node = createTestNode({ members: true })
    const client = createApiClient((request) => (new URL(request.url, 'http://mock.local').pathname.endsWith('/members') ? refused(403)() : node.handle(request)))
    renderApp({ path: '/members', node, client, enrolled: true })
    expect(await screen.findByText('This node does not report members yet.')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(membersTab()).not.toBeInTheDocument()
  })

  it('is not taken from the owner by a failure: it says so in place, and the menu shows no entry meanwhile', async () => {
    const node = createTestNode({ members: true })
    const client = createApiClient((request) => (new URL(request.url, 'http://mock.local').pathname.endsWith('/members') ? refused(500)() : node.handle(request)))
    renderApp({ path: '/members', node, client, enrolled: true })
    expect(await screen.findByRole('alert')).toBeInTheDocument()
    expect(membersTab()).not.toBeInTheDocument()
  })

  it('says an org with nothing in it so, never a count made up', async () => {
    const node = createTestNode({ members: true })
    const client = createApiClient(async (request) => {
      if (!new URL(request.url, 'http://mock.local').pathname.endsWith('/members')) return node.handle(request)
      return new Response(JSON.stringify({ org: 'acme', members: [], seats: [], externals: [] }), { headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/members', node, client, enrolled: true })
    expect(await screen.findByText('This org has no members.')).toBeInTheDocument()
    expect(screen.getByText('No seat is open.')).toBeInTheDocument()
    expect(screen.getByText('The org has no external party.')).toBeInTheDocument()
    expect(screen.getByText('Nothing waits for the root key.')).toBeInTheDocument()
  })

  it('refuses an answer with a role it cannot say, in place, and draws no one', async () => {
    const node = createTestNode({ members: true })
    const client = createApiClient(async (request) => {
      if (!new URL(request.url, 'http://mock.local').pathname.endsWith('/members')) return node.handle(request)
      return new Response(JSON.stringify({ org: 'acme', members: [{ id: 'x', label: 'X', role: 'boss', isHosted: false, devices: [] }], seats: [], externals: [] }), { headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/members', node, client, enrolled: true })
    expect(await screen.findByRole('alert')).toBeInTheDocument()
    expect(screen.queryByText('X')).not.toBeInTheDocument()
  })

  it('says a hosted member whose sign-in is not set yet, apart from one whose is', async () => {
    const { node, client } = answering({ org: 'acme', members: [{ id: 'carol', label: 'Carol', role: 'member', isHosted: true, hasSignIn: false, devices: [] }], seats: [], externals: [] })
    renderApp({ path: '/members', node, client, enrolled: true })
    expect(await screen.findByText('Hosted by this node · No sign-in is set yet')).toBeInTheDocument()
    expect(screen.queryByText(/A sign-in is set/)).not.toBeInTheDocument()
  })

  it('says who answers for a seat by name, and that the team’s owner answers when it names no one', async () => {
    const { node, client } = answering({
      org: 'acme',
      members: [{ id: 'bob', label: 'BOB', role: 'member', isHosted: false, devices: [] }],
      seats: [{ id: 'dan', label: 'Dan (finance)', askAbout: ['invoices'], answeredBy: ['bob'] }, { id: 'eve', label: 'Eve (legal)', askAbout: [], answeredBy: [] }],
      externals: [],
    })
    renderApp({ path: '/members', node, client, enrolled: true })
    const seats = await section('Open seats')
    expect(seats.getByText('Answered by BOB')).toBeInTheDocument()
    expect(seats.getByText('The team’s owner answers.')).toBeInTheDocument()
    expect(seats.getByText('Nothing to ask about is stated.')).toBeInTheDocument()
  })

  it('draws what to ask about without a character that reorders the line', async () => {
    const { node, client } = answering({ org: 'acme', members: [], seats: [{ id: 'dan', label: 'Dan', askAbout: [`in${String.fromCodePoint(0x202e)}voices`], answeredBy: [] }], externals: [] })
    const { container } = renderApp({ path: '/members', node, client, enrolled: true })
    expect(await screen.findByText('Ask about: in?voices')).toBeInTheDocument()
    expect(container.textContent).not.toContain(String.fromCodePoint(0x202e))
  })

  it('refuses an answer with a key kind it cannot say, in place', async () => {
    const { node, client } = answering({ org: 'acme', members: [{ id: 'x', label: 'X', role: 'member', isHosted: false, devices: [{ id: 'd-1', label: 'Old', keyKind: 'rsa', addedAt: '2026-11-24T12:00:00Z' }] }], seats: [], externals: [] })
    renderApp({ path: '/members', node, client, enrolled: true })
    expect(await screen.findByRole('alert')).toBeInTheDocument()
    expect(screen.queryByText('X')).not.toBeInTheDocument()
  })

  it('says a failure to read what the node says of itself in place, and does not call it a node without members', async () => {
    const node = createTestNode({ members: true })
    const client = createApiClient((request) => (new URL(request.url, 'http://mock.local').pathname === '/api/about' ? refused(500)() : node.handle(request)))
    renderApp({ path: '/members', node, client, enrolled: true })
    expect(await screen.findByRole('alert')).toBeInTheDocument()
    expect(screen.queryByText('This node does not report members yet.')).not.toBeInTheDocument()
  })

  it('asks the node nothing of what it says of itself or of members for a hosted member, who is sent to their decisions', async () => {
    const node = createTestNode({ members: true, guest: { member: 'dave', label: 'Dave' } })
    const asked: string[] = []
    const client = createApiClient((request) => {
      asked.push(new URL(request.url, 'http://mock.local').pathname)
      return node.handle(request)
    })
    const { history } = renderApp({ path: '/members', node, client, enrolled: true })
    await waitFor(() => expect(history.location.pathname).toBe('/desk'))
    expect(asked.some((path) => path.endsWith('/members'))).toBe(false)
    expect(asked).not.toContain('/api/about')
  })

  it('says it in Japanese', async () => {
    renderApp({ path: '/members', node: createTestNode({ members: true }), enrolled: true, locale: 'ja' })
    expect(await screen.findByRole('heading', { level: 1, name: '組織のメンバー' })).toBeInTheDocument()
    expect(await screen.findByText('組織のルートキーの署名待ち')).toBeInTheDocument()
    expect(screen.getByText('このノードが預かっています', { exact: false })).toBeInTheDocument()
  })
})
