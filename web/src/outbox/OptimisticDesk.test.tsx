import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { loadDrafts, draftOf } from '../desk/drafts'
import type { createMockNode } from '../mock/node'
import { renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'
import { OUTBOX_STORAGE_KEY, loadOutbox } from './storage'

const PROPOSAL = 'Add a second frontend agent'
const QUESTION = 'Which database should the app use in production?'
const TRAY = 'Sending decisions'
const ENVELOPES = /\/envelopes$/

const isEnvelope = (request: Request): boolean => request.method === 'POST' && ENVELOPES.test(new URL(request.url, 'http://mock.local').pathname)
const json = (status: number, body: unknown, headers: Record<string, string> = {}): Response => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', ...headers } })
const tray = () => screen.queryByRole('region', { name: TRAY })

/** A client for a mock node whose decisions are answered as the test says, and counted: `answer(attempt, request)` gives a response, or rejects for a lost connection. */
function clientWith(node: ReturnType<typeof createMockNode>, answer: (attempt: number, request: Request) => Promise<Response | undefined> | Response | undefined) {
  const seen: Request[] = []
  const bodies: unknown[] = []
  const client = createApiClient(async (request) => {
    if (!isEnvelope(request)) return node.handle(request)
    seen.push(request.clone())
    bodies.push(await request.clone().json())
    return (await answer(seen.length, request)) ?? node.handle(request)
  })
  return { client, seen, bodies }
}

async function approve(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('radio', { name: /Approve/ }))
  await user.click(screen.getByRole('button', { name: 'Approve with “Approve”' }))
}

describe('deciding without waiting for the node', () => {
  it('opens the next request at once, with the decided one off the pile, while the node has not answered', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    let release: () => void = () => undefined
    const held = new Promise<void>((resolve) => {
      release = resolve
    })
    const { client } = clientWith(node, async () => {
      await held
      return undefined
    })
    renderApp({ node, client, enrolled: true })
    await approve(user)
    // The node has not answered: the next request is open, the decided one is not on the pile, and the tray says it is on its way.
    expect(await screen.findByRole('heading', { level: 2, name: QUESTION })).toBeInTheDocument()
    const pile = within(screen.getByRole('listbox', { name: 'Documents waiting' }))
    expect(pile.queryByText(PROPOSAL)).not.toBeInTheDocument()
    expect(within(screen.getByRole('region', { name: TRAY })).getByText('Sending…')).toBeInTheDocument()
    expect(node.decisions()).toHaveLength(0)
    release()
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    await waitFor(() => expect(tray()).not.toBeInTheDocument())
    expect(pile.queryByText(PROPOSAL)).not.toBeInTheDocument()
  })

  it('keeps the signed decision in the browser, so that a reload resumes it with the same envelope id', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    const first = clientWith(node, () => Promise.reject(new TypeError('Failed to fetch')))
    const shown = renderApp({ node, client: first.client, enrolled: true })
    await approve(user)
    await waitFor(() => expect(loadOutbox()).toHaveLength(1))
    const [kept] = loadOutbox()
    expect(kept?.itemKey).toBe('acme:app:r-20261124-113000-c3d4')
    shown.unmount()
    expect(window.localStorage.getItem(OUTBOX_STORAGE_KEY)).not.toBeNull()

    const second = clientWith(node, () => undefined)
    renderApp({ node, client: second.client, enrolled: true })
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    expect((second.bodies[0] as { envelope: { body: { id: string } } }).envelope.body.id).toBe(kept?.id)
    await waitFor(() => expect(loadOutbox()).toEqual([]))
  })

  it('tries again by itself after a lost connection, with the same envelope, and the node keeps it once', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    const { client, bodies } = clientWith(node, (attempt) => (attempt === 1 ? Promise.reject(new TypeError('Failed to fetch')) : undefined))
    renderApp({ node, client, enrolled: true })
    await approve(user)
    expect(await screen.findByText(/Not sent yet\. Next try in under a minute\./)).toBeInTheDocument()
    expect(screen.getByText('Failed to fetch')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: `Send again: ${PROPOSAL}` }))
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    await waitFor(() => expect(tray()).not.toBeInTheDocument())
    const ids = bodies.map((body) => (body as { envelope: { body: { id: string } } }).envelope.body.id)
    expect(ids).toHaveLength(2)
    expect(new Set(ids).size).toBe(1)
  })

  it('stops for a refusal that will not change, with the node’s words, and gives the request back with what was written when it is discarded', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    const { client } = clientWith(node, () => json(409, { error: 'the team’s node refused: That request is not open.' }))
    renderApp({ node, client, enrolled: true })
    await user.click(await screen.findByRole('radio', { name: /Reject/ }))
    await user.type(screen.getByRole('textbox', { name: /^Notes/ }), 'Not now.')
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    const region = await screen.findByRole('region', { name: TRAY })
    expect(await within(region).findByText('Not sent: the team’s node refused: That request is not open.')).toBeInTheDocument()
    // The request stays off the pile while the failure is in the tray.
    expect(within(screen.getByRole('listbox', { name: 'Documents waiting' })).queryByText(PROPOSAL)).not.toBeInTheDocument()
    expect(loadOutbox()).toHaveLength(1)
    await user.click(within(region).getByRole('button', { name: `Discard: ${PROPOSAL}` }))
    await waitFor(() => expect(tray()).not.toBeInTheDocument())
    expect(loadOutbox()).toEqual([])
    expect(await within(screen.getByRole('listbox', { name: 'Documents waiting' })).findByText(PROPOSAL)).toBeInTheDocument()
    await user.click(within(screen.getByRole('listbox', { name: 'Documents waiting' })).getByText(PROPOSAL))
    expect(await screen.findByRole('radio', { name: /Reject/ })).toBeChecked()
    expect(screen.getByRole('textbox', { name: /^Notes/ })).toHaveValue('Not now.')
    expect(node.decisions()).toHaveLength(0)
  })

  it('takes a 2xx as the node having it, whether it says queued or applied, and does not bring the request back while the node holds it queued', async () => {
    const user = userEvent.setup()
    const node = createTestNode({ outcome: 'queued' })
    renderApp({ node, enrolled: true })
    await approve(user)
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    await waitFor(() => expect(tray()).not.toBeInTheDocument())
    // The node's log says it is queued, and the request is not on the pile.
    expect((await screen.findAllByText('Waiting to send')).length).toBeGreaterThan(0)
    expect(within(screen.getByRole('listbox', { name: 'Documents waiting' })).queryByText(PROPOSAL)).not.toBeInTheDocument()
  })

  it('tries again after a busy node at the time the node asked for, and not sooner', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    const { client, seen } = clientWith(node, (attempt) => (attempt === 1 ? json(503, { error: 'the node cannot keep it' }, { 'retry-after': '90' }) : undefined))
    renderApp({ node, client, enrolled: true })
    await approve(user)
    const waiting = await screen.findByText(/Not sent yet\. Next try /)
    // Asked to wait 90 seconds: not "in under a minute", which is the page's own first wait.
    expect(waiting).toHaveTextContent(/Next try in [12] minutes?\./)
    expect(waiting).not.toHaveTextContent('under a minute')
    expect(seen).toHaveLength(1)
  })

  it('does not count a decision against the waiting number, and counts it again when it is given back', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    const { client } = clientWith(node, () => json(409, { error: 'refused' }))
    renderApp({ node, client, enrolled: true })
    await screen.findByRole('heading', { level: 2, name: PROPOSAL })
    const before = document.title
    await approve(user)
    await screen.findByRole('region', { name: TRAY })
    await waitFor(() => expect(document.title).not.toBe(before))
    await user.click(screen.getByRole('button', { name: `Discard: ${PROPOSAL}` }))
    await waitFor(() => expect(document.title).toBe(before))
  })

  it('forgets the draft once the decision is kept, so that a reload does not bring the typed words back', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    const { client } = clientWith(node, () => Promise.reject(new TypeError('Failed to fetch')))
    renderApp({ node, client, enrolled: true })
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.type(screen.getByRole('textbox', { name: /^Notes/ }), 'Go.')
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    await screen.findByRole('region', { name: TRAY })
    expect(draftOf(loadDrafts(), 'acme:app:r-20261124-113000-c3d4')).toEqual({ text: '', choice: null })
  })

  it('opens the request after the one decided, not the first of the pile: deciding the second opens the third', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    renderApp({ node, enrolled: true })
    const pile = within(await screen.findByRole('listbox', { name: 'Documents waiting' }))
    await user.click(await pile.findByText(QUESTION))
    expect(await screen.findByRole('heading', { level: 2, name: QUESTION })).toBeInTheDocument()
    await user.click(await screen.findByRole('radio', { name: /SQLite/ }))
    await user.click(screen.getByRole('button', { name: /^Answer with/ }))
    expect(await screen.findByRole('heading', { level: 2, name: 'Drop support for the old export format?' })).toBeInTheDocument()
  })

  it('keeps the request off the pile while the node holds its decision queued and the team’s view still lists it', async () => {
    const user = userEvent.setup()
    const node = createTestNode({ outcome: 'queued' })
    let decided = false
    let listed: unknown
    const client = createApiClient(async (request) => {
      const path = new URL(request.url, 'http://mock.local').pathname
      if (request.method === 'GET' && path.endsWith('/teams/app/view')) {
        const answer = await node.handle(request)
        const body = (await answer.clone().json()) as { view: { requests: Array<{ id: string }> }; mine: string[] }
        // The node's own view of a team whose runtime has not applied the decision yet: the request is still listed.
        if (decided && listed !== undefined) return json(200, { ...body, view: { ...body.view, requests: [listed, ...body.view.requests] }, mine: [...body.mine, 'r-20261124-113000-c3d4'] })
        if (!decided) listed = body.view.requests.find((one) => one.id === 'r-20261124-113000-c3d4')
        return answer
      }
      if (isEnvelope(request)) decided = true
      return node.handle(request)
    })
    renderApp({ node, client, enrolled: true })
    await approve(user)
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    await waitFor(() => expect(tray()).not.toBeInTheDocument())
    expect((await screen.findAllByText('Waiting to send')).length).toBeGreaterThan(0)
    // The view lists the request again, and the log says it is queued: it stays off the pile and out of the count.
    await new Promise((resolve) => setTimeout(resolve, 200))
    expect(within(screen.getByRole('listbox', { name: 'Documents waiting' })).queryByText(PROPOSAL)).not.toBeInTheDocument()
  })

  it('leaves the decided request out of every count: the team card of the Overview, not only the desk', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    const { client } = clientWith(node, () => new Promise<Response>(() => undefined))
    renderApp({ node, client, enrolled: true })
    await user.click(await screen.findByRole('link', { name: /Overview/ }))
    const waiting = async (): Promise<number> => {
      const line = await screen.findByText(/^\d+ requests? waits? for you, in /)
      return Number(/^(\d+) request/.exec(line.textContent ?? '')?.[1])
    }
    const before = await waiting()
    expect(before).toBeGreaterThan(0)
    await user.click(screen.getByRole('link', { name: /Decisions/ }))
    await approve(user)
    await screen.findByRole('region', { name: TRAY })
    await user.click(screen.getByRole('link', { name: /Overview/ }))
    await waitFor(async () => expect(await waiting()).toBe(before - 1))
  })

  it('takes the entry out of the tray the moment the node has it, while what the node says of it is still being read', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    let release: () => void = () => undefined
    const held = new Promise<void>((resolve) => {
      release = resolve
    })
    const client = createApiClient(async (request) => {
      const path = new URL(request.url, 'http://mock.local').pathname
      // The team's read is slow, as a slow team node makes it.
      if (request.method === 'GET' && path.endsWith('/teams/app/view') && node.decisions().length > 0) await held
      return node.handle(request)
    })
    renderApp({ node, client, enrolled: true })
    await approve(user)
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    await waitFor(() => expect(tray()).not.toBeInTheDocument())
    expect(within(screen.getByRole('listbox', { name: 'Documents waiting' })).queryByText(PROPOSAL)).not.toBeInTheDocument()
    release()
  })

  it('tries what waits at once when the page is opened again, not at the time its backoff had set', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    // The node asked to wait an hour: the next try is an hour away.
    const first = clientWith(node, () => json(503, { error: 'the node cannot keep it' }, { 'retry-after': '3600' }))
    const shown = renderApp({ node, client: first.client, enrolled: true })
    await approve(user)
    await waitFor(() => expect(loadOutbox()[0]?.phase).toBe('retrying'))
    expect(Date.parse(loadOutbox()[0]?.nextAt ?? '') - Date.now()).toBeGreaterThan(3_000_000)
    shown.unmount()
    const second = clientWith(node, () => undefined)
    renderApp({ node, client: second.client, enrolled: true })
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    await waitFor(() => expect(loadOutbox()).toEqual([]))
  })
})

