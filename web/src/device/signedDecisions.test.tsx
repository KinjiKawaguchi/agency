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
  await user.type(screen.getByRole('textbox', { name: /^Notes/ }), 'Start next week.')
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

  it('signs a comment with its text and no choice', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    const keyStore = memoryKeyStore()
    await enrol(node, keyStore)
    renderApp({ node, keyStore })
    await user.type(await screen.findByLabelText(/^Notes/), 'Can we start in December instead?')
    await user.click(screen.getByRole('button', { name: 'Send a comment' }))
    await screen.findByText(/Alice commented on this/)
    const [signed] = node.envelopes()
    expect(signed?.body).toMatchObject({ kind: 'answer', target: PROPOSAL, payload: { text: 'Can we start in December instead?' } })
    expect(signed?.body.payload).not.toHaveProperty('choice')
    expect(node.decisions()[0]).not.toHaveProperty('choiceLabel')
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

  it('cannot decide while this device is not added: it says to add it, offers no send button, and keeps the draft', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    renderApp({ node, enrolled: false })
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.type(screen.getByRole('textbox', { name: /^Notes/ }), 'Start next week.')
    const prompt = await screen.findByText('Add this device to decide or comment. What you wrote is kept.')
    expect(prompt.closest('.doc-cannot-sign')?.querySelector('a')).toHaveAttribute('href', '/devices')
    expect(screen.queryByRole('button', { name: /^Approve with/ })).not.toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: /^Notes/ })).toHaveValue('Start next week.')
    expect(node.envelopes()).toHaveLength(0)
  })

  it('says so when the shortcut is used while this device is not added, and sends nothing', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    renderApp({ node, enrolled: false })
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await screen.findByText('Add this device to decide or comment. What you wrote is kept.')
    await user.keyboard('{Meta>}{Enter}{/Meta}')
    expect(await screen.findAllByText('Add this device to decide or comment. What you wrote is kept.')).toHaveLength(2)
    expect(node.envelopes()).toHaveLength(0)
    expect(screen.getByRole('radio', { name: /Approve/ })).toBeChecked()
  })

  it('lets the same draft be sent once the device is added', async () => {
    const user = userEvent.setup()
    const node = createMockNode()
    const keyStore = memoryKeyStore()
    renderApp({ node, keyStore })
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await screen.findByText('Add this device to decide or comment. What you wrote is kept.')
    await enrol(node, keyStore)
    window.dispatchEvent(new Event('visibilitychange'))
    await user.click(await screen.findByRole('button', { name: 'Approve with “Approve”' }))
    expect(await screen.findByText(/Alice approved this/)).toBeInTheDocument()
    expect(node.envelopes()).toHaveLength(1)
  })

  it('says the browser cannot sign when it cannot make keys, with no way to add a device', async () => {
    const generateKey = vi.spyOn(crypto.subtle, 'generateKey').mockRejectedValue(new Error('NotSupportedError'))
    try {
      renderApp({ node: createMockNode(), enrolled: false })
      const user = userEvent.setup()
      const prompt = await screen.findByText(/This browser cannot make the key that signs decisions\./, { selector: '.doc-cannot-sign p' })
      expect(prompt.closest('.doc-cannot-sign')?.querySelector('a')).toBeNull()
      // The shortcut says the same thing, not that the device should be added.
      await user.click(screen.getByRole('radio', { name: /Approve/ }))
      await user.keyboard('{Meta>}{Enter}{/Meta}')
      expect(await screen.findAllByText(/This browser cannot make the key that signs decisions\./)).toHaveLength(2)
    } finally {
      generateKey.mockRestore()
    }
  })

  it('does not fall back when the node refuses the signed decision, and keeps the draft', async () => {
    const user = userEvent.setup()
    const node = createMockNode({ refuseDecisionsWith: 'The team’s node says no.' })
    const keyStore = memoryKeyStore()
    await enrol(node, keyStore)
    renderApp({ node, keyStore })
    await approve(user)
    expect(await screen.findByText('The team’s node refused the decision: The team’s node says no.')).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: /^Notes/ })).toHaveValue('Start next week.')
    await waitFor(() => expect(node.envelopes()).toHaveLength(0))
  })
})
