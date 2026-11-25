import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'

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
    await screen.findByText(/It is on its way to the drafter/)
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
