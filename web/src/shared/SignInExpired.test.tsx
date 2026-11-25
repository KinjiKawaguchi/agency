import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { createTestNode } from '../test/testDevice'
import { renderApp } from '../test/render'

const AWAY = { hostname: () => 'agency.example.test', isOnline: () => true }

/** A client whose calls fail the way a redirect to another origin does, while `state.isExpired` is set. */
function clientWithSignIn(page = AWAY) {
  const node = createTestNode()
  const state = { isExpired: false }
  const client = createApiClient((request) => (state.isExpired ? Promise.reject(new TypeError('Failed to fetch')) : node.handle(request)), page)
  return { client, state, node }
}

const EXPIRED = 'Your sign-in expired. Reload to sign in again.'

describe('an expired sign-in', () => {
  it('says so, with a way to reload, instead of saying the node cannot be reached', async () => {
    const { client, state } = clientWithSignIn()
    state.isExpired = true
    renderApp({ client })
    const reload = await screen.findByRole('button', { name: 'Reload' })
    expect(reload.closest('[role="alert"]')).toHaveTextContent(EXPIRED)
    expect(screen.queryByText(/Could not reach the node/)).not.toBeInTheDocument()
  })

  it('keeps what was written, and tells the viewer when sending fails for it', async () => {
    const user = userEvent.setup()
    const { client, state } = clientWithSignIn()
    renderApp({ client })
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.type(screen.getByRole('textbox', { name: 'Notes' }), 'Start next week.')
    state.isExpired = true
    await user.click(screen.getByRole('button', { name: 'Approve with “Approve”' }))
    expect(await screen.findAllByText(EXPIRED)).not.toHaveLength(0)
    expect(screen.getByRole('textbox', { name: 'Notes' })).toHaveValue('Start next week.')
    expect(screen.queryByText(/Could not send/)).not.toBeInTheDocument()
  })

  it('goes away once a call works again', async () => {
    const { client, state } = clientWithSignIn()
    state.isExpired = true
    renderApp({ client })
    expect(await screen.findByRole('button', { name: 'Reload' })).toBeInTheDocument()
    state.isExpired = false
    window.dispatchEvent(new Event('visibilitychange'))
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Reload' })).not.toBeInTheDocument())
    expect(await screen.findByRole('option', { name: /Add a second frontend agent/ })).toBeInTheDocument()
  })

  it('is not claimed for a page on this machine, where the node is simply down', async () => {
    const { client, state } = clientWithSignIn({ hostname: () => 'localhost', isOnline: () => true })
    state.isExpired = true
    renderApp({ client })
    expect(await screen.findByText(/Could not reach/)).toHaveTextContent('Could not reach the node: Failed to fetch')
    expect(screen.queryByText(EXPIRED)).not.toBeInTheDocument()
  })
})
