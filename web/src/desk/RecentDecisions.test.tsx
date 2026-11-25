import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { renderApp } from '../test/render'
import { createTestNode, decideAsTestDevice } from '../test/testDevice'

const recent = async () => within(await screen.findByRole('region', { name: /Recently decided/ }))

describe('recently decided', () => {
  it('lists the earlier decisions under the pile, newest first, each with where it stands', async () => {
    renderApp({ path: '/desk' })
    const list = await recent()
    const rows = list.getAllByRole('listitem')
    expect(rows.map((row) => within(row).getByText(/^(Applied|Delivered|Waiting to send|Refused)$/).textContent)).toEqual(['Waiting to send', 'Refused', 'Applied', 'Delivered'])
    expect(rows[0]).toHaveTextContent('Which hosting plan should the landing page use?')
    expect(rows[0]).toHaveTextContent('Chose “Free plan”.')
    // A decision logged without a label falls back to the option's key.
    expect(rows[2]).toHaveTextContent('Chose “approve”.')
  })

  it('says when a queued decision will be tried again, and what went wrong last time', async () => {
    renderApp()
    const [queued] = (await recent()).getAllByRole('listitem')
    expect(queued).toHaveTextContent('Tried 2 times. Next try in 6 minutes.')
    expect(queued).toHaveTextContent('Last error: The team’s node did not answer.')
  })

  it.each([
    ['is a few seconds away', 5.5 * 60_000, 'Tried 2 times. Next try in under a minute.'],
    ['has passed', 7 * 60_000, 'Tried 2 times. Trying again now.'],
  ])('says so when the next try %s', async (_when, later, expected) => {
    const node = createTestNode()
    vi.useFakeTimers({ toFake: ['Date'], now: Date.now() + later })
    try {
      renderApp({ node, enrolled: true })
      const [queued] = (await recent()).getAllByRole('listitem')
      expect(queued).toHaveTextContent(expected)
    } finally {
      vi.useRealTimers()
    }
  })

  it('gives the reason a team’s node refused a decision', async () => {
    renderApp()
    const refused = (await recent()).getAllByRole('listitem')[1]
    expect(refused).toHaveTextContent('The team’s node refused it: That request was already decided.')
  })

  it('adds a decision to the top as soon as it is sent, under the request’s own title', async () => {
    const user = userEvent.setup()
    renderApp({ earlierDecisions: false })
    await screen.findByRole('article')
    expect(screen.queryByRole('region', { name: /Recently decided/ })).not.toBeInTheDocument()
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    await waitFor(async () => expect((await recent()).getAllByRole('listitem')).toHaveLength(1))
    const [added] = (await recent()).getAllByRole('listitem')
    expect(added).toHaveTextContent('Add a second frontend agent')
    expect(added).toHaveTextContent('Applied')
    expect(added).toHaveTextContent('Chose “Approve”.')
  })

  it('shows a decision the node queued as waiting to send, with when it will try again', async () => {
    const user = userEvent.setup()
    renderApp({ earlierDecisions: false, outcome: 'queued' })
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    const [added] = (await recent()).getAllByRole('listitem')
    await waitFor(() => expect(added).toHaveTextContent('Waiting to send'))
    expect(added).toHaveTextContent('Not sent yet. Next try in 5 minutes.')
    expect(added).not.toHaveTextContent('Last error')
  })

  it('folds the older decisions behind a button', async () => {
    const node = createTestNode()
    const api = createApiClient((request) => node.handle(request))
    for (const target of ['r-20261124-110000-a1b2', 'r-20261124-113000-c3d4', 'app#14']) {
      await decideAsTestDevice(api, { org: 'acme', team: 'app', answerWith: { kind: target.startsWith('app#') ? 'judge' : 'answer', target }, title: 'Earlier', text: 'Done.' })
    }
    const user = userEvent.setup()
    renderApp({ node, enrolled: true })
    const list = await recent()
    expect(await list.findAllByRole('listitem')).toHaveLength(6)
    await user.click(list.getByRole('button', { name: 'Show 1 more' }))
    expect(list.getAllByRole('listitem')).toHaveLength(7)
  })

  it('does not stop the desk when the log cannot be read, and says so', async () => {
    renderApp({ decisionLogFailsWith: 500 })
    expect(await screen.findByRole('option', { name: /Add a second frontend agent/ })).toBeInTheDocument()
    expect(await screen.findByText('Recent decisions could not be loaded: The decision log could not be read.')).toBeInTheDocument()
  })
})
