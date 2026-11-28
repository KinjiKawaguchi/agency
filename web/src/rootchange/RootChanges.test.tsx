import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { deviceKeyOf, generateKeyPair } from '../device/deviceKey'
import { createFormatters } from '../i18n/format'
import { deviceIdOf } from '../device/fingerprint'
import { createMockNode } from '../mock/node'
import { heldNode } from '../test/heldNode'
import { refreshQueries, renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'
import { answerOf, rootChangesOf } from './changes'

const AGO = (minutes: number) => new Date(Date.now() - minutes * 60_000).toISOString()
const WAITING = [
  { id: 'c1', org: 'acme', what: 'add member carol (Carol)', receivedAt: AGO(120) },
  { id: 'c2', org: 'acme', what: 'add device iPhone for bob', receivedAt: AGO(5) },
]
const STRUCTURED = [
  { id: 's1', org: 'acme', what: 'add member carol (Carol)', receivedAt: AGO(30), kind: 'addMember', member: 'carol', label: 'Carol' },
  { id: 's2', org: 'acme', what: 'add device iPhone for bob', receivedAt: AGO(20), kind: 'addDevice', member: 'bob', label: 'iPhone' },
  { id: 's3', org: 'acme', what: 'remove device d-1234 of bob', receivedAt: AGO(10), kind: 'removeDevice', member: 'bob', device: 'd-1234' },
]
const PENDING_EN = 'Approved here; it takes effect once the root key’s node signs it: run agency root-sign there.'
const PENDING_JA = 'ここでは承認しました。組織のルートキーのノードが署名すると有効になります。そのノードで agency root-sign を実行してください。'
const item = () => screen.findByRole('status', { name: /Changes waiting for the root key|組織のルートキーの署名を待っている変更/ })

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

  it('words each change from its kind and fields, in the page’s language, and not from the node’s English', async () => {
    renderApp({ node: createTestNode({ rootChanges: STRUCTURED }), path: '/desk', enrolled: true })
    const rows = within(await item()).getAllByRole('listitem')
    expect(rows[0]).toHaveTextContent('Add member Carol (carol) (Acme, asked')
    expect(rows[1]).toHaveTextContent('Add device iPhone for bob (Acme, asked')
    expect(rows[2]).toHaveTextContent('Remove device d-1234 of bob (Acme, asked')
    expect(screen.queryByText(/add member carol \(Carol\)/)).not.toBeInTheDocument()
  })

  it('words them in Japanese too, with the glossary’s words', async () => {
    renderApp({ node: createTestNode({ rootChanges: STRUCTURED }), path: '/desk', enrolled: true, locale: 'ja' })
    const rows = within(await item()).getAllByRole('listitem')
    expect(rows[0]).toHaveTextContent('メンバー Carol（carol）の追加（Acme、')
    expect(rows[1]).toHaveTextContent('bob の端末「iPhone」の追加（Acme、')
    expect(rows[2]).toHaveTextContent('bob の端末 d-1234 の削除（Acme、')
    expect(screen.queryByText(/add device iPhone for bob/)).not.toBeInTheDocument()
  })

  it('says the node’s own words for a kind it does not know, and for a known kind that lacks a field', async () => {
    const odd = [
      { id: 'o1', org: 'acme', what: 'rotate the moon', receivedAt: AGO(5), kind: 'rotateMoon' },
      { id: 'o2', org: 'acme', what: 'add member dave (Dave)', receivedAt: AGO(4), kind: 'addMember', member: 'dave' },
      { id: 'o3', org: 'acme', what: 'remove device d-9 of bob', receivedAt: AGO(3), kind: 'removeDevice', member: 'bob' },
      { id: 'o4', org: 'acme', what: 'add device Tab for bob', receivedAt: AGO(2), kind: 'addDevice', label: 'Tab' },
    ]
    renderApp({ node: createTestNode({ rootChanges: odd }), path: '/desk', enrolled: true })
    const rows = within(await item()).getAllByRole('listitem')
    expect(rows[0]).toHaveTextContent('rotate the moon (Acme, asked')
    expect(rows[1]).toHaveTextContent('add member dave (Dave) (Acme, asked')
    expect(rows[2]).toHaveTextContent('remove device d-9 of bob (Acme, asked')
    expect(rows[3]).toHaveTextContent('add device Tab for bob (Acme, asked')
  })

  it('keeps a change that comes with no `what`: worded from its kind, and plainly when there is nothing to word it from', async () => {
    const bare = [
      { id: 'n1', org: 'acme', receivedAt: AGO(30), kind: 'addMember', member: 'carol', label: 'Carol' },
      { id: 'n2', org: 'acme', receivedAt: AGO(20), kind: 'rotateMoon' },
    ]
    renderApp({ node: createTestNode({ rootChanges: bare }), path: '/desk', enrolled: true })
    const one = await item()
    expect(one).toHaveTextContent('2 changes wait for the root key’s signature.')
    const rows = within(one).getAllByRole('listitem')
    expect(rows[0]).toHaveTextContent('Add member Carol (carol) (Acme, asked')
    expect(rows[1]).toHaveTextContent('A change this page cannot describe (Acme, asked')
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
    expect(one).toHaveTextContent('組織のルートキーの署名を待っている変更が 1 件あります。')
    expect(one).toHaveTextContent('add member carol (Carol)（elsewhere、')
    expect(one).toHaveTextContent('まとめて署名するには、組織のルートキーのノードで agency root-sign を実行してください。')
  })
})

describe('an approval of a device that the root key’s node holds', () => {
  async function approve(node: ReturnType<typeof createMockNode>, locale: 'en' | 'ja' = 'en') {
    const other = await deviceKeyOf(await generateKeyPair())
    await createApiClient((request) => node.handle(request)).requestDevice({ label: 'Phone', publicKey: other.publicKey })
    renderApp({ node, path: '/devices', enrolled: true, locale })
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: locale === 'ja' ? 'コードが一致したので承認する' : 'The codes match: approve it' }))
    return user
  }

  it('is said in the page’s own words, not as a success and not as an error, and is not approved a second time', async () => {
    const node = createTestNode({ pendingRoot: true })
    await approve(node)
    const status = await screen.findByText(PENDING_EN)
    expect(status.closest('[role="status"]')).not.toBeNull()
    expect(screen.queryByText(/root key's node signs it/)).not.toBeInTheDocument()
    expect(screen.queryByText('Phone is added.')).not.toBeInTheDocument()
    expect(screen.queryByText(/Could not add the device/)).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'The codes match: approve it' })).toBeDisabled()
    expect(await screen.findByLabelText('Pairing code of Phone')).toBeInTheDocument()
  })

  it('is said in Japanese in the page’s Japanese, and the node’s English message is not shown', async () => {
    await approve(createTestNode({ pendingRoot: true }), 'ja')
    expect(await screen.findByText(PENDING_JA)).toBeInTheDocument()
    expect(screen.queryByText(/Approved here/)).not.toBeInTheDocument()
  })

  it('is still said as a success when the node applied it', async () => {
    await approve(createTestNode())
    expect(await screen.findByText('Phone is added.')).toBeInTheDocument()
    expect(screen.queryByText(PENDING_EN)).not.toBeInTheDocument()
  })

  it('shows the node’s message for an outcome the page does not know, and not a success', async () => {
    const node = createTestNode()
    const raw = node.handle
    const client = createApiClient(async (request) => (new URL(request.url, 'http://mock.local').pathname === '/api/devices/changes' ? new Response(JSON.stringify({ outcome: 'frozen', message: 'The root key’s node is closed for the night.' }), { headers: { 'content-type': 'application/json' } }) : raw(request)))
    const other = await deviceKeyOf(await generateKeyPair())
    await createApiClient((request) => node.handle(request)).requestDevice({ label: 'Phone', publicKey: other.publicKey })
    renderApp({ node, client, path: '/devices', enrolled: true })
    await userEvent.setup().click(await screen.findByRole('button', { name: 'The codes match: approve it' }))
    expect(await screen.findByText('The root key’s node is closed for the night.')).toBeInTheDocument()
    expect(screen.queryByText('Phone is added.')).not.toBeInTheDocument()
  })
})

describe('an approval the root key’s node holds, after the page is read again', () => {
  /** A node that holds `changes(device)` and has a request of another device named Phone; the page is opened afresh, with no click. */
  async function opened(changes: (device: { id: string }) => unknown[], locale: 'en' | 'ja' = 'en') {
    const other = await deviceKeyOf(await generateKeyPair())
    const node = createTestNode({ rootChanges: changes({ id: deviceIdOf(other.pairingCode) }) as never })
    await createApiClient((request) => node.handle(request)).requestDevice({ label: 'Phone', publicKey: other.publicKey })
    renderApp({ node, path: '/devices', enrolled: true, locale })
    return await screen.findByRole('button', { name: locale === 'ja' ? 'コードが一致したので承認する' : 'The codes match: approve it' })
  }
  const held = (over: object) => ({ id: 'h1', org: 'acme', receivedAt: AGO(5), kind: 'addDevice', member: 'alice', label: 'Phone', ...over })

  it('keeps the approve button off, and says why, for a request whose device id the node names', async () => {
    const approve = await opened((device) => [held({ device: device.id, label: 'Other name' })])
    expect(approve).toBeDisabled()
    expect(await screen.findByText(PENDING_EN)).toBeInTheDocument()
  })

  it('says it in Japanese', async () => {
    await opened((device) => [held({ device: device.id })], 'ja')
    expect(await screen.findByText(PENDING_JA)).toBeInTheDocument()
  })

  it('knows it by the label and the account when the node names no device', async () => {
    const approve = await opened(() => [held({})])
    expect(approve).toBeDisabled()
    expect(await screen.findByText(PENDING_EN)).toBeInTheDocument()
  })

  it.each([
    ['another device’s id, even with this label', (device: { id: string }) => [held({ device: `${device.id}0` })]],
    ['this label for another account', () => [held({ member: 'bob' })]],
    ['another label', () => [held({ label: 'Laptop' })]],
    ['a change of another kind with this label', () => [held({ kind: 'addMember' })]],
    ['nothing held', () => []],
  ])('leaves the approve button on for %s', async (_name, changes) => {
    const approve = await opened(changes)
    expect(approve).toBeEnabled()
    expect(screen.queryByText(PENDING_EN)).not.toBeInTheDocument()
  })
})

describe('how the node says what waits', () => {
  const me = (rootChanges: unknown) => ({ account: { id: 'alice', label: 'Alice', fingerprint: 'a1' }, orgs: [], funding: [], rootChanges }) as never

  it('reads a list of changes, and leaves out an entry that is not one', () => {
    expect(rootChangesOf(me(WAITING))).toEqual(WAITING)
    expect(rootChangesOf(me([null, 'x', {}, { id: 'c', org: 'acme', what: 'w' }, { id: '', org: 'acme', receivedAt: 'now' }, WAITING[0]]))).toEqual([WAITING[0]])
    for (const value of [undefined, null, 'x', 7, {}]) expect(rootChangesOf(me(value)), String(value)).toEqual([])
  })

  it('keeps an entry that has no `what`, since the page words it from its kind and fields', () => {
    expect(rootChangesOf(me([{ id: 'c', org: 'acme', receivedAt: 'now', kind: 'addMember', member: 'carol', label: 'Carol' }, { id: 'd', org: 'acme', what: '', receivedAt: 'now' }]))).toEqual([
      { id: 'c', org: 'acme', receivedAt: 'now', kind: 'addMember', member: 'carol', label: 'Carol' },
      { id: 'd', org: 'acme', receivedAt: 'now' },
    ])
  })

  it('reads kind and its fields when they are text, and leaves out what is not', () => {
    const [one] = rootChangesOf(me([{ ...STRUCTURED[1], member: 7, label: '' }]))
    expect(one).toEqual({ id: 's2', org: 'acme', what: 'add device iPhone for bob', receivedAt: STRUCTURED[1]!.receivedAt, kind: 'addDevice' })
    expect(rootChangesOf(me(STRUCTURED))).toEqual(STRUCTURED)
  })

  it('takes pending as the held change, an unknown outcome with a message as its message, and the rest as done', () => {
    expect(answerOf({ outcome: 'pending', message: 'm' })).toEqual({ status: 'pending' })
    expect(answerOf({ outcome: 'pending' })).toEqual({ status: 'pending' })
    expect(answerOf({ outcome: 'frozen', message: 'm' })).toEqual({ status: 'unknown', message: 'm' })
    expect(answerOf({ outcome: 'constructor', message: 'm' }), 'a name every object has is not a known outcome').toEqual({ status: 'unknown', message: 'm' })
    for (const value of [{ outcome: 'frozen' }, { outcome: 'frozen', message: '' }, { outcome: 'applied' }, { outcome: 'queued', message: 'm' }, { outcome: 'delivered' }, null, 'pending', undefined]) expect(answerOf(value), String(value)).toEqual({ status: 'done' })
  })
})
