import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import vectors from '../../../testdata/vectors.json' with { type: 'json' }
import { memoryKeyStore, renderApp } from '../test/render'

const CODE = /^[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}$/

describe('devices screen', () => {
  it('shows this device’s pairing code as three groups of four, and the key it comes from', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/devices' })
    const code = await screen.findByLabelText('Pairing code of this device')
    expect(code.textContent).toMatch(CODE)
    await user.click(screen.getByText('Show this device’s public key'))
    expect(screen.getByLabelText('Public key of this device').textContent).toMatch(/^[A-Za-z0-9+/]{43}=$/)
  })

  it('is reached from the viewer’s own icon, which is the current page there', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/desk' })
    await user.click(await screen.findByRole('link', { name: 'Devices' }))
    expect(await screen.findByRole('heading', { level: 1, name: 'Devices' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Devices' })).toHaveAttribute('aria-current', 'page')
  })

  it('shows the same code on a second visit, because the key is kept', async () => {
    const keyStore = memoryKeyStore()
    const first = renderApp({ path: '/devices', keyStore })
    const code = (await screen.findByLabelText('Pairing code of this device')).textContent
    first.unmount()
    renderApp({ path: '/devices', keyStore })
    expect((await screen.findByLabelText('Pairing code of this device')).textContent).toBe(code)
  })

  it('shows the code of a pasted key, to compare with the other device', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/devices' })
    await user.type(await screen.findByRole('textbox', { name: 'Public key of the other device' }), `  ${vectors.device.publicKey}\n`)
    expect(await screen.findByLabelText('Pairing code of the other device')).toHaveTextContent(vectors.device.pairingCode)
    expect(screen.getByText(/If they differ in any character, do not add it/)).toBeInTheDocument()
  })

  it('says a pasted text is not a public key, and shows no code for it', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/devices' })
    await user.type(await screen.findByRole('textbox', { name: 'Public key of the other device' }), 'hello')
    expect(await screen.findByRole('alert')).toHaveTextContent('That is not a public key.')
    expect(screen.queryByLabelText('Pairing code of the other device')).not.toBeInTheDocument()
  })

  it('removes the code again when the pasted key is cleared', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/devices' })
    const field = await screen.findByRole('textbox', { name: 'Public key of the other device' })
    await user.type(field, vectors.device.publicKey)
    await screen.findByLabelText('Pairing code of the other device')
    await user.clear(field)
    await waitFor(() => expect(screen.queryByLabelText('Pairing code of the other device')).not.toBeInTheDocument())
  })
})
