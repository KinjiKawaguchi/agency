import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { HISTORY_PAGE } from '../api/queries'
import { renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'

const historyTab = () => screen.queryByRole('link', { name: 'History' })
const versionsShown = () => screen.queryAllByText(/^Version \d+$/).map((one) => one.textContent)
const refused = (status: number) => (): Promise<Response> => Promise.resolve(new Response(JSON.stringify({ error: 'no' }), { status, headers: { 'content-type': 'application/json' } }))
const isHistory = (request: Request): boolean => new URL(request.url, 'http://mock.local').pathname.endsWith('/history')

describe('the owner’s history page', () => {
  it('draws what the node says of each version, oldest first: the sample’s kept version, the gap, and the version compared across it', async () => {
    renderApp({ path: '/history', node: createTestNode({ history: 3 }), enrolled: true })
    expect(await screen.findByRole('heading', { level: 1, name: 'History' })).toBeInTheDocument()
    await screen.findByText('Version 3')
    expect(versionsShown()).toEqual(['Version 1', 'Version 2', 'Version 3'])
    expect(screen.getByText('create the org Acme')).toBeInTheDocument()
    // Versions 1 and 3 say when they were signed; version 2, which the node does not keep, says nothing of it.
    expect(screen.getAllByText(/^signed /)).toHaveLength(2)
    expect(screen.getByText('by root key 3f9c-a1b2-77d0-e4c5')).toBeInTheDocument()
    // The node does not keep version 2: it is in its place, said so.
    expect(screen.getByText('This node does not keep this version.')).toBeInTheDocument()
    // Version 3 is counted from version 1, and its signature was not checked.
    expect(screen.getByText('Its changes are counted from version 1, since the versions between are not kept here.')).toBeInTheDocument()
    expect(screen.getByText(/^The signature was not checked/)).toBeInTheDocument()
    expect(screen.getByText('add member bob (Bob)')).toBeInTheDocument()
  })

  it('says a valid signature, an invalid one, and a fork, each as the node says it, and gives no verdict of its own', async () => {
    const node = createTestNode({ history: 3 })
    const client = createApiClient(async (request) => {
      if (!isHistory(request)) return node.handle(request)
      return new Response(
        JSON.stringify({
          entries: [
            { version: 1, at: '2026-11-24T12:00:00Z', signature: 'valid', signedBy: 'root key a', follows: true, changes: ['create the org Acme'], isKept: true },
            { version: 2, at: '2026-11-25T12:00:00Z', signature: 'invalid', follows: false, changes: ['remove member bob'], isKept: true },
          ],
        }),
        { headers: { 'content-type': 'application/json' } },
      )
    })
    renderApp({ path: '/history', node, client, enrolled: true })
    await screen.findByText('Version 2')
    expect(screen.getByText('The signature is valid')).toBeInTheDocument()
    expect(screen.getByText(/^The signature is not valid/)).toBeInTheDocument()
    expect(screen.getByText('It does not follow the version before it: the history forks here.')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Show later versions' })).not.toBeInTheDocument()
  })

  it('pages a long history from the oldest: the first page, then the versions after the last one read, joined in order, until the node says no more', async () => {
    const node = createTestNode({ history: HISTORY_PAGE + 30 })
    const asked: string[] = []
    const client = createApiClient((request) => {
      if (isHistory(request)) asked.push(new URL(request.url, 'http://mock.local').search)
      return node.handle(request)
    })
    const user = userEvent.setup()
    renderApp({ path: '/history', node, client, enrolled: true })
    await screen.findByText(`Version ${HISTORY_PAGE}`)
    expect(versionsShown()).toHaveLength(HISTORY_PAGE)
    await user.click(screen.getByRole('button', { name: 'Show later versions' }))
    await screen.findByText(`Version ${HISTORY_PAGE + 30}`)
    expect(versionsShown()).toEqual(Array.from({ length: HISTORY_PAGE + 30 }, (_none, index) => `Version ${index + 1}`))
    expect(asked).toContain(`?after=${HISTORY_PAGE}&limit=${HISTORY_PAGE}`)
    expect(screen.queryByRole('button', { name: 'Show later versions' })).not.toBeInTheDocument()
  })

  it('keeps what was read when the next page fails, says so in place, and tries that page again on a click', async () => {
    const node = createTestNode({ history: HISTORY_PAGE + 5 })
    let isDown = true
    const client = createApiClient((request) => (isHistory(request) && new URL(request.url, 'http://mock.local').searchParams.has('after') && isDown ? refused(500)() : node.handle(request)))
    const user = userEvent.setup()
    renderApp({ path: '/history', node, client, enrolled: true })
    await screen.findByText(`Version ${HISTORY_PAGE}`)
    await user.click(screen.getByRole('button', { name: 'Show later versions' }))
    expect(await screen.findByRole('alert')).toHaveTextContent(/^Could not read later versions/)
    expect(versionsShown()).toHaveLength(HISTORY_PAGE)
    isDown = false
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    await screen.findByText(`Version ${HISTORY_PAGE + 5}`)
    expect(versionsShown()).toHaveLength(HISTORY_PAGE + 5)
    expect(screen.queryByText(/^Could not read later versions/)).not.toBeInTheDocument()
  })

  it('is in the menu, under the organization, for the owner', async () => {
    renderApp({ path: '/history', node: createTestNode({ history: 3 }), enrolled: true })
    await screen.findByText('Version 3')
    await waitFor(() => expect(historyTab()).toBeInTheDocument())
  })

  it('does not ask a node that does not list the capability, says it does not report the history, and has no entry in the menu', async () => {
    const node = createTestNode()
    const asked: string[] = []
    const client = createApiClient((request) => {
      asked.push(new URL(request.url, 'http://mock.local').pathname)
      return node.handle(request)
    })
    renderApp({ path: '/history', node, client, enrolled: true })
    expect(await screen.findByText('This node does not report the history yet.')).toBeInTheDocument()
    expect(asked.some((path) => path.endsWith('/history'))).toBe(false)
    expect(historyTab()).not.toBeInTheDocument()
  })

  it('shows nothing, says nothing and has no menu entry for anyone the node answers 403 (not the owner)', async () => {
    const node = createTestNode({ history: 3 })
    const client = createApiClient((request) => (isHistory(request) ? refused(403)() : node.handle(request)))
    renderApp({ path: '/history', node, client, enrolled: true })
    expect(await screen.findByText('This node does not report the history yet.')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(historyTab()).not.toBeInTheDocument()
  })

  it('is not taken from the owner by a failure: it says so in place, and the menu shows no entry meanwhile', async () => {
    const node = createTestNode({ history: 3 })
    const client = createApiClient((request) => (isHistory(request) ? refused(500)() : node.handle(request)))
    renderApp({ path: '/history', node, client, enrolled: true })
    expect(await screen.findByRole('alert')).toBeInTheDocument()
    expect(historyTab()).not.toBeInTheDocument()
  })

  it('says a failure to read what the node says of itself in place, and does not call it a node without a history', async () => {
    const node = createTestNode({ history: 3 })
    const client = createApiClient((request) => (new URL(request.url, 'http://mock.local').pathname === '/api/about' ? refused(500)() : node.handle(request)))
    renderApp({ path: '/history', node, client, enrolled: true })
    expect(await screen.findByRole('alert')).toBeInTheDocument()
    expect(screen.queryByText('This node does not report the history yet.')).not.toBeInTheDocument()
  })

  it('says a history with no version so, never a count made up', async () => {
    const node = createTestNode({ history: 3 })
    const client = createApiClient(async (request) => (isHistory(request) ? new Response(JSON.stringify({ entries: [] }), { headers: { 'content-type': 'application/json' } }) : node.handle(request)))
    renderApp({ path: '/history', node, client, enrolled: true })
    expect(await screen.findByText('The node keeps no version of this org.')).toBeInTheDocument()
  })

  it('draws a change without a character that reorders the line', async () => {
    const node = createTestNode({ history: 3 })
    const client = createApiClient(async (request) => (isHistory(request) ? new Response(JSON.stringify({ entries: [{ version: 1, changes: [`add member bo${String.fromCodePoint(0x202e)}b`], isKept: true }] }), { headers: { 'content-type': 'application/json' } }) : node.handle(request)))
    const { container } = renderApp({ path: '/history', node, client, enrolled: true })
    expect(await screen.findByText('add member bo?b')).toBeInTheDocument()
    expect(container.textContent).not.toContain(String.fromCodePoint(0x202e))
  })

  it('asks the node nothing of what it says of itself or of the history for a hosted member, who is sent to their decisions', async () => {
    const node = createTestNode({ history: 3, guest: { member: 'dave', label: 'Dave' } })
    const asked: string[] = []
    const client = createApiClient((request) => {
      asked.push(new URL(request.url, 'http://mock.local').pathname)
      return node.handle(request)
    })
    const { history } = renderApp({ path: '/history', node, client, enrolled: true })
    await waitFor(() => expect(history.location.pathname).toBe('/desk'))
    expect(asked.some((path) => path.endsWith('/history'))).toBe(false)
    expect(asked).not.toContain('/api/about')
  })

  it('says it in Japanese', async () => {
    renderApp({ path: '/history', node: createTestNode({ history: 3 }), enrolled: true, locale: 'ja' })
    expect(await screen.findByRole('heading', { level: 1, name: '組織の変更履歴' })).toBeInTheDocument()
    expect(await screen.findByText('版 3')).toBeInTheDocument()
    expect(screen.getByText('このノードは、この版を保管していません。')).toBeInTheDocument()
    expect(within(document.body).getByText(/^署名は確かめていません/)).toBeInTheDocument()
  })
})
