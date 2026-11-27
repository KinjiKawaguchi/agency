import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import { draftOf, loadDrafts } from './drafts'

describe('drafts', () => {
  it('survive a reload of the page, so signing in again does not cost what was written', async () => {
    const user = userEvent.setup()
    const first = renderApp()
    await user.click(await screen.findByRole('radio', { name: /Reject/ }))
    await user.type(screen.getByRole('textbox', { name: /^Notes/ }), 'Not this quarter.')
    first.unmount()

    renderApp()
    expect(await screen.findByRole('textbox', { name: /^Notes/ })).toHaveValue('Not this quarter.')
    expect(screen.getByRole('radio', { name: /Reject/ })).toBeChecked()
  })

  it('are forgotten once sent', async () => {
    const user = userEvent.setup()
    const first = renderApp()
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.type(screen.getByRole('textbox', { name: /^Notes/ }), 'Go.')
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    // The decision is kept and the desk has moved on: what was typed is forgotten, so that a reload does not bring it back.
    await waitFor(() => expect(draftOf(loadDrafts(), 'acme:app:r-20261124-113000-c3d4')).toEqual({ text: '', choice: null }))
    first.unmount()

    renderApp()
    expect(await screen.findByRole('textbox', { name: /^Notes/ })).toHaveValue('')
  })

  it('are ignored when what is stored is not drafts', async () => {
    window.sessionStorage.setItem('agency.drafts', '{"not":"a list"}')
    renderApp()
    expect(await screen.findByRole('textbox', { name: /^Notes/ })).toHaveValue('')
  })
})
