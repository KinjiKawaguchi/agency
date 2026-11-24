import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import { withRequestFor } from '../test/views'

const PROPOSAL = 'Add a second frontend agent'
const QUESTION = 'Which database should the app use in production?'
const PENDING = 'Drop support for the old export format?'

const pile = async () => within(await screen.findByRole('listbox', { name: 'Documents waiting' }))

describe('decision desk', () => {
  it('summarizes the situation and opens the most urgent document first', async () => {
    renderApp()
    expect(await screen.findByRole('heading', { level: 1 })).toHaveTextContent(`1 person is waiting for your answer.Start by approving “${PROPOSAL}”.`)
    const paper = screen.getByRole('article')
    expect(within(paper).getByRole('heading', { level: 2 })).toHaveTextContent(PROPOSAL)
    expect(within(paper).getByRole('group', { name: 'Drafted by Lead, Approved by Alice' })).toBeInTheDocument()
  })

  it('groups the pile and lists a request for someone else apart', async () => {
    renderApp({ view: withRequestFor('frontend') })
    const headings = (await pile()).getAllByRole('heading', { level: 2 }).map((heading) => heading.textContent)
    expect(headings).toEqual(['Proposals for you to approve1', 'Questions for you1', 'Decisions waiting on the board1', 'Waiting on someone else1'])
  })

  it('sends the chosen option and the note with the request’s own answer target, then stamps the document', async () => {
    const user = userEvent.setup()
    const { node } = renderApp()
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.type(screen.getByRole('textbox', { name: 'Notes' }), 'Start next week.')
    await user.click(screen.getByRole('button', { name: 'Approve with “Approve”' }))

    expect(await screen.findByText(/Alice approved this\. It is on its way to the drafter\./)).toBeInTheDocument()
    expect(node.decisions()).toEqual([
      { org: 'acme', team: 'app', answerWith: { kind: 'answer', target: 'r-20261124-113000-c3d4' }, choice: 'approve', text: 'Start next week.' },
    ])
  })

  it('answers a pending decision as a judgement on its work item', async () => {
    const user = userEvent.setup()
    const { node } = renderApp()
    await user.click(await screen.findByRole('option', { name: new RegExp(PENDING) }))
    await user.type(screen.getByRole('textbox', { name: 'Notes' }), 'Drop it in the next release.')
    await user.click(screen.getByRole('button', { name: 'Send decision' }))
    await waitFor(() =>
      expect(node.decisions()).toEqual([{ org: 'acme', team: 'app', answerWith: { kind: 'judge', target: 'app#14' }, text: 'Drop it in the next release.' }]),
    )
  })

  it('moves the answered document out of the pile once the node no longer lists it', async () => {
    const user = userEvent.setup()
    renderApp()
    await user.click(await screen.findByRole('radio', { name: /Reject/ }))
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    const list = await pile()
    await waitFor(() => expect(list.queryByText(PROPOSAL)).not.toBeInTheDocument(), { timeout: 4000 })
    expect(screen.getByRole('article')).toHaveTextContent(QUESTION)
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
    await user.type(await screen.findByRole('textbox', { name: 'Notes' }), 'j2k')
    expect(screen.getByRole('textbox', { name: 'Notes' })).toHaveValue('j2k')
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
    const { node } = renderApp({ view: withRequestFor('frontend') })
    await user.click(await screen.findByRole('option', { name: /nightly export include archived projects/ }))
    expect(screen.getByText(/Waiting for Frontend to decide/)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Answer' })).not.toBeInTheDocument()
    await user.keyboard('1')
    await user.keyboard('{Meta>}{Enter}{/Meta}')
    expect(node.decisions()).toEqual([])
  })

  it('keeps the draft and reports the failure when the node refuses the answer', async () => {
    const user = userEvent.setup()
    const { node } = renderApp()
    await user.click(await screen.findByRole('radio', { name: /Reject/ }))
    // Answer the request behind the viewer's back, so the node no longer has it.
    await node.handle(
      new Request('http://mock.local/api/orgs/acme/teams/app/decisions', {
        method: 'POST',
        body: JSON.stringify({ kind: 'answer', target: 'r-20261124-113000-c3d4', text: 'x' }),
      }),
    )
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    expect(await screen.findByText('Could not send: That request is not open.')).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: /Reject/ })).toBeChecked()
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
    expect(screen.getByRole('heading', { level: 4, name: 'Proposal' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 4, name: 'Cost' })).toBeInTheDocument()
  })

  it('shows the board as the drafter of a pending decision', async () => {
    const user = userEvent.setup()
    renderApp()
    await user.click(await screen.findByRole('option', { name: new RegExp(PENDING) }))
    expect(screen.getByRole('group', { name: 'Drafted by board, Answered by Alice' })).toBeInTheDocument()
  })
})
