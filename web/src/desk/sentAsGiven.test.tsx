import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'
import { withRequestFor } from '../test/views'

const REQUEST = 'r-20261124-120000-e5f6'
const GIVEN_TITLE = 'Export \u202Earchived\u202C projects?'
const GIVEN_OPTION = 'Include\u200B them'

function viewWithHiddenCharacters() {
  const view = withRequestFor('owner')
  return {
    ...view,
    requests: view.requests.map((request) =>
      request.id === REQUEST
        ? { ...request, title: GIVEN_TITLE, options: request.options.map((option) => (option.key === 'yes' ? { ...option, text: GIVEN_OPTION } : option)) }
        : request,
    ),
  }
}

describe('what is sent back with a decision is what the node gave, not what was shown', () => {
  it('shows the title and the choice without the hidden characters, and sends them as the node gave them', async () => {
    const user = userEvent.setup()
    const node = createTestNode({ view: viewWithHiddenCharacters() })
    renderApp({ node, path: `/desk?item=acme:app:${REQUEST}`, enrolled: true })
    const heading = await screen.findByRole('heading', { level: 2, name: 'Export ?archived? projects?' })
    expect(heading).toBeInTheDocument()
    const choice = await screen.findByRole('radio', { name: /Include\? them/ })
    await user.click(choice)
    await user.click(within(document.body).getByRole('button', { name: /Include\? them/ }))
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    expect(node.decisions()[0]).toMatchObject({ title: GIVEN_TITLE, choiceLabel: GIVEN_OPTION })
  })
})
