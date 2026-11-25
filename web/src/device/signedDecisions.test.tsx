import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { createMockNode } from '../mock/node'
import { memoryKeyStore, renderApp } from '../test/render'
import { settledDeviceKey } from './useDeviceKey'

const PROPOSAL = 'r-20261124-113000-c3d4'

async function enrol(node: ReturnType<typeof createMockNode>, keyStore: ReturnType<typeof memoryKeyStore>): Promise<string> {
  const held = await settledDeviceKey(keyStore)
  if (held.status !== 'ready') throw new Error('no key')
  await createApiClient((request) => node.handle(request)).requestDevice({ label: 'Browser', publicKey: held.key.publicKey })
  node.addDeviceFromTerminal(held.key.pairingCode)
  return `d-${held.key.pairingCode.replaceAll('-', '')}`
}

async function approve(user: ReturnType<typeof userEvent.setup>): Promise<void> {
  await user.click(await screen.findByRole('radio', { name: /Approve/ }))
  await user.type(screen.getByRole('textbox', { name: 'Notes' }), 'Start next week.')
  await user.click(screen.getByRole('button', { name: 'Approve with “Approve”' }))
}

describe('deciding with the device key', () => {
  it('signs the decision with the key and sends it as an envelope the node accepts, notes beside it', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    const keyStore = memoryKeyStore()
    const deviceId = await enrol(node, keyStore)
    renderApp({ node, keyStore })
    await approve(user)
    await screen.findByText(/Alice approved this/)
    expect(node.envelopes()).toHaveLength(1)
    const [signed] = node.envelopes()
    expect(signed).toMatchObject({ signer: deviceId, body: { kind: 'answer', org: 'acme', team: 'app', target: PROPOSAL, account: 'alice', payload: { text: 'Start next week.', choice: 'approve' } } })
    expect(signed?.body.id).toMatch(/^[0-9a-f]{32}$/)
    expect(signed?.body.at).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/)
    expect(node.decisions()).toMatchObject([{ answerWith: { target: PROPOSAL }, title: 'Add a second frontend agent', choice: 'approve', choiceLabel: 'Approve' }])
  })

  it('shows the signed decision in the log with the title and the label of its choice', async () => {
    const user = userEvent.setup()
    const node = createMockNode({ earlierDecisions: false })
    const keyStore = memoryKeyStore()
    await enrol(node, keyStore)
    renderApp({ node, keyStore })
    await approve(user)
    const row = (await screen.findByRole('region', { name: /Recently decided/ })).querySelector('li')
    expect(row).toHaveTextContent('Add a second frontend agent')
    expect(row).toHaveTextContent('Chose “Approve”.')
  })

  it('falls back to the node signing, while this device is not added yet, so the owner can still decide', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    renderApp({ node })
    await approve(user)
    await screen.findByText(/Alice approved this/)
    expect(node.envelopes()).toHaveLength(0)
    expect(node.decisions()).toHaveLength(1)
  })

  it('does not fall back when the node refuses the signed decision, and keeps the draft', async () => {
    const user = userEvent.setup()
    const node = createMockNode({ refuseDecisionsWith: 'The team’s node says no.' })
    const keyStore = memoryKeyStore()
    await enrol(node, keyStore)
    renderApp({ node, keyStore })
    await approve(user)
    expect(await screen.findByText('The team’s node refused the decision: The team’s node says no.')).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'Notes' })).toHaveValue('Start next week.')
    await waitFor(() => expect(node.envelopes()).toHaveLength(0))
  })
})
