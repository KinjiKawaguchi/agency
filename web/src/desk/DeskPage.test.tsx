import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { renderApp } from '../test/render'
import { createTestNode, decideAsTestDevice } from '../test/testDevice'
import { withRequestFor } from '../test/views'

const PROPOSAL = 'Add a second frontend agent'
const QUESTION = 'Which database should the app use in production?'
const PENDING = 'Drop support for the old export format?'

const pile = async () => within(await screen.findByRole('listbox', { name: 'Documents waiting' }))

describe('decision desk filtered to a team', () => {
  it('shows only that team’s requests, and says so, with a way back to all of them', async () => {
    renderApp({ path: '/desk?team=lab%2Fnotebook', moreOrgs: true })
    expect(await screen.findByText('Showing only Notebook.')).toBeInTheDocument()
    expect(await screen.findByRole('heading', { level: 2, name: 'Publish the notebook under the lab’s name?' })).toBeInTheDocument()
    expect(screen.queryByText(PROPOSAL)).not.toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Show all teams' })).toHaveAttribute('href', '/desk')
  })

  it('shows every team’s requests when the team asked for is not one of the viewer’s', async () => {
    renderApp({ path: '/desk?team=nowhere%2Fnone' })
    expect(await screen.findByRole('heading', { level: 2, name: PROPOSAL })).toBeInTheDocument()
    expect(screen.queryByText(/Showing only/)).not.toBeInTheDocument()
  })
})

describe('decision desk', () => {
  it('summarizes the situation and opens the most urgent document first', async () => {
    renderApp()
    // The heading appears once the account is known; its text settles when every team has loaded.
    await waitFor(() => expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(`1 participant is waiting for your answer.Start by approving “${PROPOSAL}”.`))
    const paper = screen.getByRole('article')
    expect(within(paper).getByRole('heading', { level: 2 })).toHaveTextContent(PROPOSAL)
    expect(within(paper).getByRole('group', { name: 'Drafted by Lead, Approved by Alice' })).toBeInTheDocument()
  })

  it('groups the pile and lists a request for someone else apart', async () => {
    renderApp({ view: withRequestFor('frontend') })
    const headings = (await pile()).getAllByRole('heading', { level: 2 }).map((heading) => heading.textContent)
    expect(headings).toEqual(['Proposals for you to approve1', 'Questions for you1', 'Pending decisions on the board1', 'Waiting on someone else1'])
  })

  it('sends the chosen option and the note with the request’s own answer target, and opens the next document at once', async () => {
    const user = userEvent.setup()
    const { node } = renderApp()
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.type(screen.getByRole('textbox', { name: /^Notes/ }), 'Start next week.')
    await user.click(screen.getByRole('button', { name: 'Approve with “Approve”' }))

    expect(await screen.findByRole('heading', { level: 2, name: QUESTION })).toBeInTheDocument()
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    expect(node.decisions()).toEqual([
      { org: 'acme', team: 'app', answerWith: { kind: 'answer', target: 'r-20261124-113000-c3d4' }, title: PROPOSAL, choice: 'approve', choiceLabel: 'Approve', text: 'Start next week.' },
    ])
  })

  it('sends text with no option picked as a comment, which decides nothing', async () => {
    const user = userEvent.setup()
    const { node } = renderApp()
    await user.type(await screen.findByLabelText(/^Notes/), 'Can we start in December instead?')
    expect(screen.queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Send a comment' }))

    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    expect(node.decisions()).toEqual([
      { org: 'acme', team: 'app', answerWith: { kind: 'answer', target: 'r-20261124-113000-c3d4' }, title: PROPOSAL, text: 'Can we start in December instead?' },
    ])
    expect(node.decisions()[0]).not.toHaveProperty('choice')
    expect(node.decisions()[0]).not.toHaveProperty('choiceLabel')
  })

  it('reads as a decision again once an option is picked, and as the plain verb while nothing is written', async () => {
    const user = userEvent.setup()
    renderApp()
    expect(await screen.findByRole('button', { name: 'Approve' })).toBeInTheDocument()
    await user.type(screen.getByLabelText(/^Notes/), 'Soon.')
    expect(screen.getByRole('button', { name: 'Send a comment' })).toBeInTheDocument()
    await user.click(screen.getByRole('radio', { name: /Approve/ }))
    expect(screen.getByRole('button', { name: 'Approve with “Approve”' })).toBeInTheDocument()
  })

  it('calls text on a question with options a comment too, and a question without options answers with it', async () => {
    const user = userEvent.setup()
    renderApp()
    await user.click(await screen.findByRole('option', { name: new RegExp(QUESTION) }))
    expect(screen.getByRole('button', { name: 'Answer' })).toBeInTheDocument()
    await user.type(screen.getByLabelText(/^Notes/), 'Depends on the budget.')
    expect(screen.getByRole('button', { name: 'Send a comment' })).toBeInTheDocument()
    await user.click(screen.getByRole('option', { name: new RegExp(PENDING) }))
    // This one has no options, so there is nothing to pick: what is written is the answer.
    await user.type(screen.getByLabelText('Your answer'), 'Later.')
    expect(screen.getByRole('button', { name: 'Send decision' })).toBeInTheDocument()
  })

  it('names the box above it, not only inside it', async () => {
    renderApp()
    const box = await screen.findByRole('textbox', { name: /^Notes/ })
    const label = document.querySelector(`label[for="${box.id}"]`)
    expect(label).toHaveTextContent('Notes (optional). With no option picked, they are sent as a comment.')
    expect(box).toHaveAttribute('placeholder', 'Add conditions or notes (optional)')
  })

  it('answers a pending decision as a judgement on its work item', async () => {
    const user = userEvent.setup()
    const { node } = renderApp()
    await user.click(await screen.findByRole('option', { name: new RegExp(PENDING) }))
    await user.type(screen.getByRole('textbox', { name: 'Your answer' }), 'Drop it in the next release.')
    await user.click(screen.getByRole('button', { name: 'Send decision' }))
    await waitFor(() =>
      expect(node.decisions()).toEqual([{ org: 'acme', team: 'app', answerWith: { kind: 'judge', target: 'app#14' }, title: PENDING, text: 'Drop it in the next release.' }]),
    )
  })

  it('takes the answered document off the pile at once, and keeps it off once the node has it', async () => {
    const user = userEvent.setup()
    renderApp()
    await user.click(await screen.findByRole('radio', { name: /Reject/ }))
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    const list = await pile()
    expect(list.queryByText(PROPOSAL)).not.toBeInTheDocument()
    expect(screen.getByRole('article')).toHaveTextContent(QUESTION)
    await waitFor(() => expect(screen.queryByRole('region', { name: 'Sending decisions' })).not.toBeInTheDocument())
    expect(list.queryByText(PROPOSAL)).not.toBeInTheDocument()
  })

  it('picks options with number keys and sends with Cmd+Enter', async () => {
    const user = userEvent.setup()
    const { node } = renderApp()
    await screen.findByRole('article')
    await user.keyboard('2')
    expect(screen.getByRole('radio', { name: /Reject/ })).toBeChecked()
    await user.keyboard('{Meta>}{Enter}{/Meta}')
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    expect(node.decisions()[0]).toMatchObject({ answerWith: { target: 'r-20261124-113000-c3d4' }, choice: 'reject' })
  })

  it('moves through the pile with j and k', async () => {
    const user = userEvent.setup()
    renderApp()
    await screen.findByRole('article')
    await user.keyboard('j')
    expect(screen.getByRole('heading', { level: 2, name: QUESTION })).toBeInTheDocument()
    await user.keyboard('k')
    expect(screen.getByRole('heading', { level: 2, name: PROPOSAL })).toBeInTheDocument()
  })

  it('does not treat keys typed into the note as shortcuts', async () => {
    const user = userEvent.setup()
    renderApp()
    await user.type(await screen.findByRole('textbox', { name: /^Notes/ }), 'j2k')
    expect(screen.getByRole('textbox', { name: /^Notes/ })).toHaveValue('j2k')
    expect(screen.getByRole('heading', { level: 2, name: PROPOSAL })).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: /Reject/ })).not.toBeChecked()
  })

  it('refuses to send an empty answer', async () => {
    const user = userEvent.setup()
    const { node } = renderApp()
    await user.click(await screen.findByRole('button', { name: 'Approve' }))
    expect(await screen.findByText('Pick an option or write an answer first.')).toBeInTheDocument()
    expect(node.decisions()).toEqual([])
  })

  it('shows a request for someone else as read-only', async () => {
    const user = userEvent.setup()
    // A draft is kept for it (as a reload brings back), so that only the rule that it is not the viewer's to decide stops it from being sent.
    window.sessionStorage.setItem('agency.drafts', JSON.stringify([['acme:app:r-20261124-120000-e5f6', { text: 'Include them.', choice: 'yes' }]]))
    const { node } = renderApp({ view: withRequestFor('frontend') })
    await user.click(await screen.findByRole('option', { name: /nightly export include archived projects/ }))
    expect(screen.getByText(/Waiting for Frontend to decide/)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Answer' })).not.toBeInTheDocument()
    await user.keyboard('1')
    await user.keyboard('{Meta>}{Enter}{/Meta}')
    await new Promise((resolve) => setTimeout(resolve, 200))
    expect(node.decisions()).toEqual([])
    expect(screen.queryByText(/Sent\./)).not.toBeInTheDocument()
  })

  it('keeps a decision the node refused in the tray, and discarding it gives back the request and what was chosen', async () => {
    const user = userEvent.setup()
    const node = createTestNode()
    renderApp({ node, enrolled: true })
    await user.click(await screen.findByRole('radio', { name: /Reject/ }))
    // Answer the request behind the viewer's back, so the node no longer has it.
    await decideAsTestDevice(createApiClient((request) => node.handle(request)), { org: 'acme', team: 'app', answerWith: { kind: 'answer', target: 'r-20261124-113000-c3d4' }, title: PROPOSAL, text: 'x' })
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    // The request is off the pile and the failure is in the tray with the node's words; discarding gives it back with what was chosen.
    const tray = await screen.findByRole('region', { name: 'Sending decisions' })
    expect(await within(tray).findByText('Not sent: That request is not open.')).toBeInTheDocument()
    await user.click(within(tray).getByRole('button', { name: `Discard: ${PROPOSAL}` }))
    await user.click(await (await pile()).findByText(PROPOSAL))
    expect(await screen.findByRole('radio', { name: /Reject/ })).toBeChecked()
  })

  it('warns when a team’s data is a copy that could not be refreshed', async () => {
    renderApp({ siteError: 'connection refused' })
    expect(await screen.findByText(/Showing a copy of this team from .*Its node could not be reached: connection refused/)).toBeInTheDocument()
  })

  it('says nothing about a copy that was refreshed fine', async () => {
    renderApp()
    await screen.findByRole('article')
    expect(screen.queryByText(/Showing a copy/)).not.toBeInTheDocument()
  })

  it('takes a queued decision as the node having it', async () => {
    const user = userEvent.setup()
    const { node } = renderApp({ outcome: 'queued' })
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    // The node took it (queued): the tray is empty and the request stays off the pile while the node's log says it is queued.
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    await waitFor(() => expect(screen.queryByRole('region', { name: 'Sending decisions' })).not.toBeInTheDocument())
    expect((await screen.findAllByText('Waiting to send')).length).toBeGreaterThan(0)
    expect((await pile()).queryByText(PROPOSAL)).not.toBeInTheDocument()
  })

  it('keeps a refused decision in the tray with the node’s words, and gives the draft back when it is discarded', async () => {
    const user = userEvent.setup()
    renderApp({ refuseDecisionsWith: 'The decision has expired.' })
    await user.click(await screen.findByRole('radio', { name: /Reject/ }))
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    const tray = await screen.findByRole('region', { name: 'Sending decisions' })
    expect(await within(tray).findByText('Not sent: The decision has expired.')).toBeInTheDocument()
    await user.click(within(tray).getByRole('button', { name: `Discard: ${PROPOSAL}` }))
    await user.click(await (await pile()).findByText(PROPOSAL))
    expect(await screen.findByRole('radio', { name: /Reject/ })).toBeChecked()
  })

  it('shows earlier messages on a request', async () => {
    const user = userEvent.setup()
    renderApp()
    await user.click(await screen.findByRole('option', { name: new RegExp(QUESTION) }))
    expect(screen.getByText('What does the load test cover?')).toBeInTheDocument()
    expect(screen.getByText('Ten times the current peak traffic for one hour.')).toBeInTheDocument()
  })

  it('says so when the node cannot be reached', async () => {
    renderApp({ failWith: 503 })
    expect(await screen.findByRole('alert')).toHaveTextContent('Could not reach the node: The node is unavailable.')
  })

  it('shows the proposal body as formatted text, with its headings below the title (', async () => {
    const user = userEvent.setup()
    renderApp()
    await user.click(await screen.findByText(/Read the document/))
    expect(await screen.findByRole('heading', { level: 4, name: 'Proposal' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 4, name: 'Cost' })).toBeInTheDocument()
  })

  it('shows the board as the drafter of a pending decision', async () => {
    const user = userEvent.setup()
    renderApp()
    await user.click(await screen.findByRole('option', { name: new RegExp(PENDING) }))
    expect(screen.getByRole('group', { name: 'Drafted by board, Answered by Alice' })).toBeInTheDocument()
  })
})
