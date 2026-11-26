import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'

const card = async (label: string) => within(await screen.findByRole('article', { name: label }))

/** A client that counts the writes and can answer them with a status of its own. */
function clientOf(node: ReturnType<typeof createTestNode>, refuseWith?: { status: number; message: string }) {
  const writes: unknown[] = []
  const client = createApiClient(async (request) => {
    if (request.method === 'POST' && new URL(request.url, 'http://mock.local').pathname.endsWith('/outcomes/observations')) {
      writes.push(await request.clone().json())
      if (refuseWith !== undefined) return new Response(JSON.stringify({ error: refuseWith.message }), { status: refuseWith.status, headers: { 'content-type': 'application/json' } })
    }
    return node.handle(request)
  })
  return { client, writes }
}

async function open(label: string) {
  const user = userEvent.setup()
  const revenue = await card(label)
  await user.click(revenue.getByRole('button', { name: 'Enter a number' }))
  const form = within(await screen.findByRole('form', { name: `Enter a number: ${label}` }))
  return { user, form, revenue }
}

describe('entering a number', () => {
  it('sends the number for the day, says it was saved, and the card shows it as the latest', async () => {
    const node = createTestNode({ outcomes: true })
    const { client, writes } = clientOf(node)
    renderApp({ path: '/outcomes', node, client, enrolled: true })
    const { user, form, revenue } = await open('Monthly revenue')
    expect(form.getByLabelText('Day')).toHaveValue('2026-11-24')
    await user.type(form.getByLabelText('Number'), '1400000')
    await user.type(form.getByLabelText('Note (optional)'), 'Month closed')
    await user.click(form.getByRole('button', { name: 'Save' }))
    expect(await screen.findByText('Saved ¥1,400,000 for 2026-11-24.')).toBeInTheDocument()
    expect(writes).toEqual([{ kpi: 'revenue', date: '2026-11-24', value: 1400000, note: 'Month closed' }])
    await waitFor(() => expect(revenue.getByText('¥1,400,000', { selector: '.oc-value' })).toBeInTheDocument())
    expect(revenue.getByText(/Rose \+¥200,000 since Oct 31/)).toBeInTheDocument()
  })

  it('says a mistake before sending it, and sends nothing', async () => {
    const node = createTestNode({ outcomes: true })
    const { client, writes } = clientOf(node)
    renderApp({ path: '/outcomes', node, client, enrolled: true })
    const { user, form } = await open('Monthly revenue')
    await user.type(form.getByLabelText('Number'), 'a lot')
    await user.click(form.getByRole('button', { name: 'Save' }))
    expect(await screen.findByText(/The number must be written with digits/)).toBeInTheDocument()
    await user.clear(form.getByLabelText('Number'))
    await user.type(form.getByLabelText('Number'), '5')
    await user.clear(form.getByLabelText('Day'))
    await user.type(form.getByLabelText('Day'), '2030-01-01')
    await user.click(form.getByRole('button', { name: 'Save' }))
    expect(await screen.findByText(/The day must be a real day, at most one day ahead of today/)).toBeInTheDocument()
    expect(writes).toEqual([])
  })

  it.each([
    [409, 'The node keeps no more.', 'This measure already has 5,000 numbers, the most the node keeps.'],
    [403, 'no', 'Only the owner can enter numbers.'],
    [400, 'The value is not a number.', 'The node refused it: The value is not a number.'],
    [404, 'No such measure.', 'The node refused it: No such measure.'],
  ])('says what the node answered when it refuses with %i', async (status, message, shown) => {
    const node = createTestNode({ outcomes: true })
    renderApp({ path: '/outcomes', node, client: clientOf(node, { status, message }).client, enrolled: true })
    const { user, form } = await open('Monthly revenue')
    await user.type(form.getByLabelText('Number'), '5')
    await user.click(form.getByRole('button', { name: 'Save' }))
    expect(await screen.findByText(shown)).toBeInTheDocument()
    expect(screen.queryByText(/^Saved /)).not.toBeInTheDocument()
  })

  it('says in which unit to write the number, for each unit', async () => {
    renderApp({ path: '/outcomes', node: createTestNode({ outcomes: true }), enrolled: true })
    const revenue = await open('Monthly revenue')
    expect(revenue.form.getByText(/In JPY\./)).toBeInTheDocument()
    expect(revenue.form.getByText(/A second number for the same day replaces the first/)).toBeInTheDocument()
    const activation = await open('Activation')
    expect(activation.form.getByText(/In percent: 41\.5 is 41\.5%\./)).toBeInTheDocument()
  })

  it('reads in Japanese', async () => {
    renderApp({ path: '/outcomes', node: createTestNode({ outcomes: true }), enrolled: true, locale: 'ja' })
    const user = userEvent.setup()
    const revenue = await card('Monthly revenue')
    await user.click(revenue.getByRole('button', { name: '数値を入れる' }))
    expect(await screen.findByLabelText('日付')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '保存' })).toBeInTheDocument()
  })
})

describe('the history of a KPI', () => {
  it('is not read until it is opened, then lists every entry newest first with where each came from', async () => {
    const node = createTestNode({ outcomes: true })
    const paths: string[] = []
    const client = createApiClient((request) => (paths.push(new URL(request.url, 'http://mock.local').pathname), node.handle(request)))
    renderApp({ path: '/outcomes', node, client, enrolled: true })
    const user = userEvent.setup()
    const activation = await card('Activation')
    expect(paths.filter((path) => path.endsWith('/outcomes/observations'))).toEqual([])
    await user.click(activation.getByRole('button', { name: 'Show history' }))
    const list = within(await screen.findByRole('list', { name: 'History: Activation' }))
    const rows = list.getAllByRole('listitem').map((row) => row.textContent ?? '')
    expect(rows).toHaveLength(2)
    expect(rows[0]).toContain('Nov 24')
    expect(rows[0]).toContain('43.2%')
    expect(rows[0]).toContain('From analytics-export')
    expect(rows[1]).toContain('41.5%')
    await user.click(activation.getByRole('button', { name: 'Hide history' }))
    expect(screen.queryByRole('list', { name: 'History: Activation' })).not.toBeInTheDocument()
  })

  it('keeps the first of two entries for a day in the history, marked as replaced, while the later one wins', async () => {
    const node = createTestNode({ outcomes: true })
    renderApp({ path: '/outcomes', node, enrolled: true })
    const { user, form, revenue } = await open('Monthly revenue')
    await user.clear(form.getByLabelText('Day'))
    await user.type(form.getByLabelText('Day'), '2026-11-20')
    await user.type(form.getByLabelText('Number'), '1360000')
    await user.click(form.getByRole('button', { name: 'Save' }))
    await waitFor(() => expect(revenue.getByText('¥1,360,000', { selector: '.oc-value' })).toBeInTheDocument())
    await user.click(revenue.getByRole('button', { name: 'Show history' }))
    const list = within(await screen.findByRole('list', { name: 'History: Monthly revenue' }))
    const rows = list.getAllByRole('listitem')
    const replaced = rows.filter((row) => row.textContent?.includes('Replaced by a later entry'))
    expect(replaced).toHaveLength(1)
    expect(replaced[0]).toHaveTextContent('¥1,350,000')
    expect(rows.find((row) => row.textContent?.includes('¥1,360,000'))).not.toHaveTextContent('Replaced')
  })

  it('says when nothing has been entered for a KPI', async () => {
    renderApp({ path: '/outcomes', node: createTestNode({ outcomes: true }), enrolled: true })
    const user = userEvent.setup()
    const lead = await card('Lead time of a feature')
    await user.click(lead.getByRole('button', { name: 'Show history' }))
    expect(await lead.findAllByText('No number has been entered yet.')).not.toHaveLength(0)
  })

  it('refreshes the readings and an open history after a number is saved, without asking again', async () => {
    const node = createTestNode({ outcomes: true })
    const paths: string[] = []
    const client = createApiClient((request) => (paths.push(`${request.method} ${new URL(request.url, 'http://mock.local').pathname}`), node.handle(request)))
    renderApp({ path: '/outcomes', node, client, enrolled: true })
    const { user, form, revenue } = await open('Monthly revenue')
    await user.click(revenue.getByRole('button', { name: 'Show history' }))
    await screen.findByRole('list', { name: 'History: Monthly revenue' })
    const readingsBefore = paths.filter((path) => path.endsWith('/outcomes/readings')).length
    const historyBefore = paths.filter((path) => path.includes('/outcomes/observations') && path.startsWith('GET')).length
    await user.type(form.getByLabelText('Number'), '1500000')
    await user.click(form.getByRole('button', { name: 'Save' }))
    await waitFor(() => expect(within(screen.getByRole('list', { name: 'History: Monthly revenue' })).getAllByRole('listitem')).toHaveLength(3))
    expect(paths.filter((path) => path.endsWith('/outcomes/readings')).length, 'the readings are read again').toBeGreaterThan(readingsBefore)
    expect(paths.filter((path) => path.includes('/outcomes/observations') && path.startsWith('GET')).length, 'the open history is read again').toBeGreaterThan(historyBefore)
  })
})
