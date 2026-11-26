import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'
import { teamOf, withRequestFor } from '../test/views'
import { toTeam } from '../api/teams'
import { openItems } from './items'

const REQUEST = 'r-20261124-120000-e5f6'
const TITLE = 'Should the nightly export include archived projects?'
const nodeFor = (label: string) => createTestNode({ view: withRequestFor('finance'), relayedFor: { [REQUEST]: label } })
const pileRow = async () => (await screen.findAllByText(TITLE)).map((one) => one.closest('button')).find((one) => one !== null) as HTMLElement

describe('a request addressed to a seat or an external party the viewer answers for', () => {
  it('says whom it is for, on the pile and on the paper, in the words of the glossary', async () => {
    renderApp({ node: nodeFor('Erin (outside counsel)'), path: `/desk?item=acme:app:${REQUEST}`, enrolled: true })
    const row = within(await pileRow())
    expect(row.getByText('Answer for Erin (outside counsel)')).toBeInTheDocument()
    const heading = await screen.findByRole('heading', { level: 2, name: TITLE })
    expect(within(heading.parentElement as HTMLElement).getByText('Answer for Erin (outside counsel)')).toBeInTheDocument()
    expect(screen.getAllByText(/^Answer for /)).toHaveLength(2)
  })

  it('says it in Japanese with the glossary’s word, 代理で答える', async () => {
    renderApp({ node: nodeFor('Erin'), path: '/desk', enrolled: true, locale: 'ja' })
    expect(within(await pileRow()).getByText('Erin の代理で答える')).toBeInTheDocument()
  })

  it('shows the label as text: a label with markup is not markup', async () => {
    renderApp({ node: nodeFor('<b>Erin</b>'), path: '/desk', enrolled: true })
    expect(within(await pileRow()).getByText('Answer for <b>Erin</b>')).toBeInTheDocument()
    expect(document.querySelector('.pile-for b')).toBeNull()
  })

  it('says nothing of it for a request that is the viewer’s own', async () => {
    renderApp({ node: nodeFor('Erin'), path: '/desk', enrolled: true })
    await pileRow()
    expect(screen.queryAllByText(/^Answer for /)).toHaveLength(1)
    const own = (await screen.findAllByText('Add a second frontend agent')).map((one) => one.closest('button')).find((one) => one !== null) as HTMLElement
    expect(within(own).queryByText(/^Answer for /)).not.toBeInTheDocument()
  })

  it('sends whom it answers for with the decision, and the log says it answered for them', async () => {
    const user = userEvent.setup()
    const node = nodeFor('Erin (outside counsel)')
    renderApp({ node, path: `/desk?item=acme:app:${REQUEST}`, enrolled: true })
    await user.click(await screen.findByRole('radio', { name: /Include them/ }))
    await user.click(screen.getByRole('button', { name: /Include them/ }))
    await waitFor(() => expect(node.decisions()).toHaveLength(1))
    expect(node.decisions()).toMatchObject([{ answerWith: { target: REQUEST }, relayedFor: 'Erin (outside counsel)' }])
    const log = await createApiClient((request) => node.handle(request)).decisions(10)
    expect(log.entries[0]).toMatchObject({ relayedFor: 'Erin (outside counsel)' })
    expect(await screen.findByText('Answered for Erin (outside counsel).')).toBeInTheDocument()
  })

  it('does not send a `relayedFor` for a decision that is the viewer’s own', async () => {
    const user = userEvent.setup()
    const node = nodeFor('Erin')
    renderApp({ node, path: '/desk', enrolled: true })
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.click(screen.getByRole('button', { name: 'Approve with “Approve”' }))
    await screen.findByText(/Alice approved this/)
    expect(node.decisions()[0]).not.toHaveProperty('relayedFor')
  })
})

describe('how the snapshot says whom a request is for', () => {
  it('is kept as the label for each request id, and is nothing when the node leaves it out', () => {
    const base = { view: withRequestFor('finance'), viewer: 'owner', mine: [REQUEST], waitingOnViewer: [], source: { kind: 'local' as const } }
    const [item] = openItems([toTeam({ ...base, relayedFor: { [REQUEST]: 'Erin' } })]).filter((one) => one.request.id === REQUEST)
    expect(item?.relayedFor).toBe('Erin')
    const [bare] = openItems([toTeam(base)]).filter((one) => one.request.id === REQUEST)
    expect(bare?.relayedFor).toBeUndefined()
    expect(openItems([teamOf(withRequestFor('finance'), 'owner', [REQUEST], { [REQUEST]: 'Erin' })]).find((one) => one.request.id === REQUEST)?.relayedFor).toBe('Erin')
  })
})
