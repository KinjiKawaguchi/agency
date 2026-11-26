import { screen, waitFor, within } from '@testing-library/react'
import { createMockNode } from '../mock/node'
import { createFormatters } from '../i18n/format'
import { heldNode } from '../test/heldNode'
import { refreshQueries, renderApp } from '../test/render'
import { recoveriesOf } from './provisional'

const AT = '2026-12-01T09:30:00Z'
const STARTED = { takesEffectAt: AT, newRootKey: 'b7c8d9e0f1a2', replacesRecoveryKey: true }
const notice = () => screen.findByRole('alert', { name: /Recovery of an org|組織のルート鍵の回復/ })
const noticeOf = async (text: string) => (await screen.findAllByRole('alert', { name: /Recovery of an org|組織のルート鍵の回復/ })).find((one) => one.textContent?.includes(text))

describe('a recovery of an org that waits', () => {
  it('is at the top of the desk, says when it takes effect, and gives the command that cancels it with the org’s id', async () => {
    renderApp({ node: createMockNode({ recoveries: { acme: STARTED } }), path: '/desk', enrolled: true })
    const one = await notice()
    const time = createFormatters('en').dateTime(AT)
    expect(one).toHaveTextContent(`A recovery of Acme was started. It takes effect at ${time}.`)
    expect(one).toHaveTextContent('If this wasn’t you, cancel it on the root key’s node: agency root-cancel-recovery --org acme')
    expect(within(one).getByText('agency root-cancel-recovery --org acme').tagName).toBe('CODE')
    expect(one).toHaveTextContent('New root key: b7c8d9e0f1a2')
    expect(one).toHaveTextContent('It also replaces the recovery key.')
    const briefing = (await screen.findAllByRole('heading', { level: 1 }))[0]
    expect(one.compareDocumentPosition(briefing as Node) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })

  it('cannot be dismissed: it has no button and no link to close it, and a symbol that is not the only sign', async () => {
    renderApp({ node: createMockNode({ recoveries: { acme: STARTED } }), path: '/desk', enrolled: true })
    const one = await notice()
    expect(within(one).queryAllByRole('button')).toHaveLength(0)
    expect(within(one).queryAllByRole('link')).toHaveLength(0)
    expect(one.querySelector('[aria-hidden="true"]')).toHaveTextContent('⚠')
    expect(one).toHaveTextContent('A recovery of Acme was started.')
  })

  it('leaves out the time when the node kept no record of the start, and the lines that do not apply', async () => {
    renderApp({ node: createMockNode({ recoveries: { acme: {} } }), path: '/desk', enrolled: true })
    const one = await notice()
    expect(one.textContent).toBe('⚠︎A recovery of Acme was started.If this wasn’t you, cancel it on the root key’s node: agency root-cancel-recovery --org acme')
  })

  it('is one for each org that has a recovery, the label for people and the id for the command', async () => {
    renderApp({ node: createMockNode({ moreOrgs: true, recoveries: { acme: {}, lab: { takesEffectAt: AT } } }), path: '/desk', enrolled: true })
    expect(await noticeOf('Acme')).toBeDefined()
    expect(await noticeOf('Lab')).toHaveTextContent('--org lab')
    expect(screen.getAllByRole('alert', { name: /Recovery of an org/ })).toHaveLength(2)
  })

  it('is not there for an org with none', async () => {
    renderApp({ node: createMockNode({ moreOrgs: true, recoveries: { lab: {} } }), path: '/desk', enrolled: true })
    await noticeOf('Lab')
    expect(screen.queryByText(/A recovery of Acme/)).not.toBeInTheDocument()
    expect(screen.queryByText(/A recovery of Club/)).not.toBeInTheDocument()
  })

  it('shows nothing before the node has answered, and then it is there', async () => {
    const node = createMockNode({ recoveries: { acme: STARTED } })
    const { client, release } = heldNode(node, ['/api/me'])
    renderApp({ node, client, path: '/desk', enrolled: true })
    expect(await screen.findByText('Loading…')).toBeInTheDocument()
    expect(screen.queryByRole('alert', { name: /Recovery of an org/ })).not.toBeInTheDocument()
    release()
    expect(await notice()).toBeInTheDocument()
  })

  it('goes when the node stops saying it waits: it was cancelled', async () => {
    const node = createMockNode({ recoveries: { acme: STARTED } })
    renderApp({ node, path: '/desk', enrolled: true })
    await notice()
    node.cancelRecovery('acme')
    await refreshQueries(['me'])
    await waitFor(() => expect(screen.queryByRole('alert', { name: /Recovery of an org/ })).not.toBeInTheDocument())
  })

  it('is not there when no org has one', async () => {
    renderApp({ path: '/desk', enrolled: true })
    await screen.findByRole('heading', { level: 1 })
    expect(screen.queryByRole('alert', { name: /Recovery of an org/ })).not.toBeInTheDocument()
  })

  it('says it in Japanese with the glossary’s words, and keeps the command whole', async () => {
    renderApp({ node: createMockNode({ recoveries: { acme: STARTED } }), path: '/desk', enrolled: true, locale: 'ja' })
    const one = await notice()
    const time = createFormatters('ja').dateTime(AT)
    expect(one).toHaveTextContent(`Acme のルート鍵の回復が始まりました。${time} に有効になります。`)
    expect(one).toHaveTextContent('新しいルート鍵: b7c8d9e0f1a2')
    expect(one).toHaveTextContent('回復用の鍵も取り替えます。')
    expect(one).toHaveTextContent('心当たりがないときは、ルート鍵のノードで取り消してください: agency root-cancel-recovery --org acme')
  })
})

describe('how the node says a recovery waits', () => {
  const org = { id: 'acme', label: 'Acme', isRoot: true, teams: [] }
  const me = (recovery: unknown) => ({ account: { id: 'alice', label: 'Alice', fingerprint: 'a1' }, orgs: [{ ...org, recovery }], funding: [] })

  it('reads an object, and nothing else counts', () => {
    expect(recoveriesOf(me({}) as never)).toEqual([{ org: { id: 'acme', label: 'Acme' }, replacesRecoveryKey: false }])
    for (const recovery of [undefined, null, 'soon', 7, true]) expect(recoveriesOf(me(recovery) as never), String(recovery)).toEqual([])
  })

  it('takes a time and a key only when they are text, and the replacement only when it is true', () => {
    expect(recoveriesOf(me({ takesEffectAt: AT, newRootKey: 'ab', replacesRecoveryKey: true }) as never)[0]).toEqual({ org: { id: 'acme', label: 'Acme' }, takesEffectAt: AT, newRootKey: 'ab', replacesRecoveryKey: true })
    expect(recoveriesOf(me({ takesEffectAt: '', newRootKey: 5, replacesRecoveryKey: 'yes' }) as never)[0]).toEqual({ org: { id: 'acme', label: 'Acme' }, replacesRecoveryKey: false })
  })
})
