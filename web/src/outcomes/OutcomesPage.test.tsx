import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { heldNode } from '../test/heldNode'
import { refreshQueries, renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'

const card = async (label: string) => within(await screen.findByRole('article', { name: label }))
const outcomesTab = () => screen.queryByRole('link', { name: 'Outcomes' })

describe('the owner’s outcomes page', () => {
  it('is not in the menu, shows no error, and leaves for the overview, for anyone but the owner (the node answers 403)', async () => {
    renderApp({ path: '/outcomes', enrolled: true })
    expect(await screen.findByRole('heading', { level: 1, name: 'Overview' })).toBeInTheDocument()
    expect(outcomesTab()).not.toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.queryByText(/Outcomes/)).not.toBeInTheDocument()
  })

  it('is not taken from the owner by a failure: the page stays, says so, and tries again, and the menu shows no entry meanwhile', async () => {
    const node = createTestNode({ outcomes: true })
    let isDown = true
    const client = createApiClient((request) => (isDown && new URL(request.url, 'http://mock.local').pathname.includes('/outcomes') ? Promise.resolve(new Response(JSON.stringify({ error: 'The node is busy.' }), { status: 500, headers: { 'content-type': 'application/json' } })) : node.handle(request)))
    const user = userEvent.setup()
    renderApp({ path: '/outcomes', node, client, enrolled: true })
    expect(await screen.findByRole('alert')).toHaveTextContent('The node is busy.')
    expect(screen.queryByRole('heading', { level: 1, name: 'Overview' }), 'the owner is not sent away').not.toBeInTheDocument()
    expect(outcomesTab()).not.toBeInTheDocument()
    isDown = false
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(await screen.findByRole('heading', { level: 1, name: 'Outcomes' })).toBeInTheDocument()
    expect(outcomesTab()).toBeInTheDocument()
  })

  it('stays through a read that fails after it has answered: the page and the menu entry are kept', async () => {
    const node = createTestNode({ outcomes: true })
    let isDown = false
    const client = createApiClient((request) => (isDown ? Promise.resolve(new Response(JSON.stringify({ error: 'The node is restarting.' }), { status: 500, headers: { 'content-type': 'application/json' } })) : node.handle(request)))
    renderApp({ path: '/outcomes', node, client, enrolled: true })
    expect(await screen.findByRole('heading', { level: 1, name: 'Outcomes' })).toBeInTheDocument()
    isDown = true
    await refreshQueries()
    await refreshQueries(['me'])
    await refreshQueries(['outcomes'])
    expect(screen.getByRole('heading', { level: 1, name: 'Outcomes' })).toBeInTheDocument()
    expect(outcomesTab()).toBeInTheDocument()
    expect(screen.queryByText('The node is restarting.')).not.toBeInTheDocument()
    expect(screen.getByRole('article', { name: 'Monthly revenue' })).toBeInTheDocument()
  })

  it('treats 404 like 403: not the owner’s, nothing to show', async () => {
    const node = createTestNode({ outcomes: true })
    const client = createApiClient((request) => (new URL(request.url, 'http://mock.local').pathname.includes('/outcomes') ? Promise.resolve(new Response(JSON.stringify({ error: 'No such organization.' }), { status: 404, headers: { 'content-type': 'application/json' } })) : node.handle(request)))
    renderApp({ path: '/outcomes', node, client, enrolled: true })
    expect(await screen.findByRole('heading', { level: 1, name: 'Overview' })).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('is in the Results menu once the owner’s read has answered, and not before', async () => {
    const node = createTestNode({ outcomes: true })
    const { client, release } = heldNode(node, ['/outcomes'])
    renderApp({ path: '/costs', node, client, enrolled: true })
    await screen.findByRole('navigation', { name: 'Results pages' })
    expect(outcomesTab()).not.toBeInTheDocument()
    release()
    expect(await screen.findByRole('link', { name: 'Outcomes' })).toHaveAttribute('href', '/outcomes')
  })

  it('shows a placeholder, no title and no number, while the owner’s read is made', async () => {
    const node = createTestNode({ outcomes: true })
    const { client, release } = heldNode(node, ['/outcomes'])
    renderApp({ path: '/outcomes', node, client, enrolled: true })
    await waitFor(() => expect(document.querySelector('.skeleton')).not.toBeNull())
    expect(screen.queryByRole('heading', { level: 1, name: 'Outcomes' })).not.toBeInTheDocument()
    release()
    expect(await screen.findByRole('heading', { level: 1, name: 'Outcomes' })).toBeInTheDocument()
  })

  it('draws each KPI: the latest number in its own unit, how it moved from the baseline, and a gap where there is no number', async () => {
    renderApp({ path: '/outcomes', node: createTestNode({ outcomes: true }), enrolled: true })
    const revenue = await card('Monthly revenue')
    expect(revenue.getByText('¥1,350,000', { selector: '.oc-value' })).toBeInTheDocument()
    expect(revenue.getByText(/Rose \+¥150,000 since Oct 31 \(¥1,200,000\)\./)).toBeInTheDocument()
    expect(revenue.getByText('better')).toBeInTheDocument()
    expect(revenue.getByText('Higher is better.')).toBeInTheDocument()
    const activation = await card('Activation')
    expect(activation.getByText('43.2%', { selector: '.oc-value' })).toBeInTheDocument()
    expect(activation.queryByText(/since/), 'no baseline, no movement').not.toBeInTheDocument()
    const lead = await card('Lead time of a feature')
    expect(lead.getByText('No number has been entered yet.')).toBeInTheDocument()
    expect(lead.queryByText(/^0/), 'an empty KPI is not a 0').not.toBeInTheDocument()
    expect(document.querySelectorAll('.oc-kpi .ev-line').length, 'a line for each KPI that has numbers').toBe(2)
  })

  it('reads in the order of the sample, with every state of a reading in plain words and numbers only for the ready ones', async () => {
    renderApp({ path: '/outcomes', node: createTestNode({ outcomes: true }), enrolled: true })
    expect(await screen.findByRole('heading', { level: 2, name: 'Readings' })).toBeInTheDocument()
    const text = document.body.textContent ?? ''
    expect(text).toContain('Before it shipped 40% (Oct 15) → after it 44% (Nov 10)')
    expect(text).toContain('Rose +4 points (+10%)')
    expect(text).toContain('The figure moved after the work shipped. That does not say the work moved it.')
    expect(text).toContain('Cannot be told apart from: app#12, which shipped in the same span.')
    expect(text).toContain('Not shipped yet: nothing to compare.')
    expect(text).toContain('The day it shipped is not known, so nothing is guessed.')
    expect(text).toContain('No number in the 90 days before it shipped. Enter one dated on or before Nov 1.')
    expect(text).toContain('No number in the 90 days after it shipped, so there is nothing to compare, and there will not be.')
    expect(text).toContain('Too early: a change needs 14 days to show. It can be read from Dec 4.')
    expect(text).toContain('Not enough numbers: 2 of 3 around the day it shipped.')
    expect(text, 'never "because" or "caused"').not.toMatch(/because|caused|cause of/i)
    // Only the two ready readings have the words that go with numbers.
    expect(document.querySelectorAll('.oc-reading').length).toBe(8)
    expect(document.querySelectorAll('.oc-reading .oc-move').length).toBe(2)
  })

  it('opens on 90 days, offers 30, and asks the node for the period chosen', async () => {
    const user = userEvent.setup()
    const node = createTestNode({ outcomes: true })
    renderApp({ path: '/outcomes', node, enrolled: true })
    const ninety = await screen.findByRole('button', { name: '90 days' })
    expect(ninety).toHaveAttribute('aria-pressed', 'true')
    expect(screen.queryByRole('button', { name: '14 days' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '30 days' }))
    expect(await screen.findByRole('button', { name: '30 days' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: '90 days' })).toHaveAttribute('aria-pressed', 'false')
  })

  it('says the figures are for reading a movement, not for accounting', async () => {
    renderApp({ path: '/outcomes', node: createTestNode({ outcomes: true }), enrolled: true })
    expect(await screen.findByText('The figures are for reading how a measure moves, not for accounting.')).toBeInTheDocument()
  })

  it('reads in Japanese, with the wording of the glossary', async () => {
    renderApp({ path: '/outcomes', node: createTestNode({ outcomes: true }), enrolled: true, locale: 'ja' })
    expect(await screen.findByRole('heading', { level: 1, name: '成果の指標' })).toBeInTheDocument()
    expect(await screen.findByRole('heading', { level: 2, name: '読み取り' })).toBeInTheDocument()
    expect(document.body.textContent).toContain('完了のあとで数値が動きました。この仕事が動かしたとは言えません。')
    expect(document.body.textContent).toContain('完了した日が分からないため、比べられません。')
    expect(document.body.textContent).toContain('完了の前後の数値が足りません: 2 / 3')
    expect(document.body.textContent, 'it says only that the work did not necessarily move it').not.toMatch(/動かしました|原因は/)
  })
})
