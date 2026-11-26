import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { deviceKeyOf, generateKeyPair } from '../device/deviceKey'
import { createFormatters } from '../i18n/format'
import { createMockNode, PENDING_ROOT_MESSAGE } from '../mock/node'
import { heldNode } from '../test/heldNode'
import { refreshQueries, renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'
import { pendingOf, rootChangesOf } from './provisional'

const AGO = (minutes: number) => new Date(Date.now() - minutes * 60_000).toISOString()
const WAITING = [
  { id: 'c1', org: 'acme', what: 'add member carol (Carol)', receivedAt: AGO(120) },
  { id: 'c2', org: 'acme', what: 'add device iPhone for bob', receivedAt: AGO(5) },
]
const item = () => screen.findByRole('status', { name: /Changes waiting for the root key|ルート鍵の署名を待っている変更/ })

describe('the changes that wait for the root key’s signature, on the desk', () => {
  it('is one item: how many, what each changes and for which org, and the command that signs them together', async () => {
    renderApp({ node: createTestNode({ rootChanges: WAITING }), path: '/desk', enrolled: true })
    const one = await item()
    expect(one).toHaveTextContent('2 changes wait for the root key’s signature.')
    const rows = within(one).getAllByRole('listitem')
    expect(rows).toHaveLength(2)
    expect(rows[0]).toHaveTextContent(`add member carol (Carol) (Acme, asked ${createFormatters('en').relative(WAITING[0]?.receivedAt ?? '', 'just now')})`)
    expect(rows[1]).toHaveTextContent('add device iPhone for bob (Acme, asked')
    expect(one).toHaveTextContent('Sign them together on the root key’s node: run agency root-sign there.')
    expect(within(one).getByText('agency root-sign').tagName).toBe('CODE')
  })

  it('says one change in the singular', async () => {
    renderApp({ node: createTestNode({ rootChanges: [WAITING[0]!] }), path: '/desk', enrolled: true })
    expect(await item()).toHaveTextContent('1 change waits for the root key’s signature.')
  })

  it('shows nothing before the node has answered, and nothing when no change waits', async () => {
    const node = createTestNode({ rootChanges: WAITING })
    const { client, release } = heldNode(node, ['/api/me'])
    renderApp({ node, client, path: '/desk', enrolled: true })
    expect(await screen.findByText('Loading…')).toBeInTheDocument()
    expect(screen.queryByRole('status', { name: /Changes waiting/ })).not.toBeInTheDocument()
    release()
    await item()
  })

  it('is not there when none waits', async () => {
    renderApp({ path: '/desk', enrolled: true })
    await screen.findByRole('heading', { level: 1 })
    expect(screen.queryByRole('status', { name: /Changes waiting/ })).not.toBeInTheDocument()
  })

  it('goes when the node stops saying it: the owner signed them', async () => {
    const node = createTestNode({ rootChanges: WAITING })
    renderApp({ node, path: '/desk', enrolled: true })
    await item()
    node.signRootChanges()
    await refreshQueries(['me'])
    await waitFor(() => expect(screen.queryByRole('status', { name: /Changes waiting/ })).not.toBeInTheDocument())
  })

  it('names an org the viewer does not list by its id, and says it all in Japanese', async () => {
    renderApp({ node: createTestNode({ rootChanges: [{ ...WAITING[0]!, org: 'elsewhere' }] }), path: '/desk', enrolled: true, locale: 'ja' })
    const one = await item()
    expect(one).toHaveTextContent('ルート鍵の署名を待っている変更が 1 件あります。')
    expect(one).toHaveTextContent('add member carol (Carol)（elsewhere、')
    expect(one).toHaveTextContent('まとめて署名するには、ルート鍵のノードで agency root-sign を実行してください。')
  })
})

describe('an approval of a device that the root key’s node holds', () => {
  async function approve(node: ReturnType<typeof createMockNode>) {
    const other = await deviceKeyOf(await generateKeyPair())
    await createApiClient((request) => node.handle(request)).requestDevice({ label: 'Phone', publicKey: other.publicKey })
    renderApp({ node, path: '/devices', enrolled: true })
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: 'The codes match: approve it' }))
    return user
  }

  it('is said with the node’s message, not as a success and not as an error, and is not approved a second time', async () => {
    const node = createTestNode({ pendingRoot: true })
    await approve(node)
    const status = await screen.findByText(PENDING_ROOT_MESSAGE)
    expect(status.closest('[role="status"]')).not.toBeNull()
    expect(screen.queryByText('Phone is added.')).not.toBeInTheDocument()
    expect(screen.queryByText(/Could not add the device/)).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'The codes match: approve it' })).toBeDisabled()
    expect(await screen.findByLabelText('Pairing code of Phone')).toBeInTheDocument()
  })

  it('is still said as a success when the node applied it', async () => {
    await approve(createTestNode())
    expect(await screen.findByText('Phone is added.')).toBeInTheDocument()
    expect(screen.queryByText(PENDING_ROOT_MESSAGE)).not.toBeInTheDocument()
  })

  it('says it in the words of the page when the node sent a pending with no message', async () => {
    const node = createTestNode({ pendingRoot: true })
    const raw = node.handle
    const client = createApiClient(async (request) => (new URL(request.url, 'http://mock.local').pathname === '/api/devices/changes' ? new Response(JSON.stringify({ outcome: 'pending' }), { headers: { 'content-type': 'application/json' } }) : raw(request)))
    const other = await deviceKeyOf(await generateKeyPair())
    await createApiClient((request) => node.handle(request)).requestDevice({ label: 'Phone', publicKey: other.publicKey })
    renderApp({ node, client, path: '/devices', enrolled: true })
    await userEvent.setup().click(await screen.findByRole('button', { name: 'The codes match: approve it' }))
    expect(await screen.findByText('Approved here; it takes effect once the root key’s node signs it: run agency root-sign there.')).toBeInTheDocument()
    expect(screen.queryByText('Phone is added.')).not.toBeInTheDocument()
  })
})

describe('how the node says what waits', () => {
  const me = (rootChanges: unknown) => ({ account: { id: 'alice', label: 'Alice', fingerprint: 'a1' }, orgs: [], funding: [], rootChanges }) as never

  it('reads a list of changes, and leaves out an entry that is not one', () => {
    expect(rootChangesOf(me(WAITING))).toEqual(WAITING)
    expect(rootChangesOf(me([null, 'x', {}, { id: 'c', org: 'acme', what: '', receivedAt: 'now' }, { id: 'c', org: 'acme', what: 'w' }, WAITING[0]]))).toEqual([WAITING[0]])
    for (const value of [undefined, null, 'x', 7, {}]) expect(rootChangesOf(me(value)), String(value)).toEqual([])
  })

  it('takes only an outcome of pending as one, with its message when it has one', () => {
    expect(pendingOf({ outcome: 'pending', message: 'm' })).toEqual({ outcome: 'pending', message: 'm' })
    expect(pendingOf({ outcome: 'pending' })).toEqual({ outcome: 'pending' })
    expect(pendingOf({ outcome: 'pending', message: '' })).toEqual({ outcome: 'pending' })
    for (const value of [{ outcome: 'applied' }, { outcome: 'queued', message: 'm' }, { outcome: 'delivered' }, null, 'pending', undefined]) expect(pendingOf(value), String(value)).toBeUndefined()
  })
})
