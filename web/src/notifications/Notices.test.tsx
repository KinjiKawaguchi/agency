import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { createMockNode } from '../mock/node'
import { renderApp } from '../test/render'
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
