import { expect, test } from '@playwright/test'
import { decisionsTaken, enrolBrowser, resetNode, trackErrors } from './helpers'

const PROPOSAL = 'Add a second frontend agent'
const QUESTION = 'Which database should the app use in production?'
const PROPOSAL_ID = 'r-20261124-113000-c3d4'

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  await resetNode(request)
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

test('opens the most urgent request first, from a page that carries the node’s token and security headers', async ({ page }) => {
  const response = await page.goto('/desk')
  expect(response?.headers()['content-security-policy']).toContain("script-src 'self'")
  await expect(page.locator('meta[name="agency-token"]')).toHaveAttribute('content', 'mock-token')
  await expect(page.getByRole('heading', { level: 1 })).toContainText('1 participant is waiting for your answer.')
  await expect(page.getByRole('article').getByRole('heading', { level: 2 })).toHaveText(PROPOSAL)
  await expect(page.getByRole('group', { name: 'Drafted by Lead, Approved by Alice' })).toBeVisible()
})

test('decides a proposal: picks an option, moves on at once, and the node has it', async ({ page, request }) => {
  await enrolBrowser(page, request)
  await page.goto('/desk')
  const paper = page.getByRole('article')
  await paper.locator('label.choice').filter({ hasText: 'Approve' }).click()
  await paper.getByRole('textbox', { name: /^Notes/ }).fill('Start next week.')
  await paper.getByRole('button', { name: 'Approve with “Approve”' }).click()

  // The answered request leaves the pile and the next one opens at once; the decision is sent from the tray.
  await expect(page.getByRole('listbox').getByText(PROPOSAL)).toHaveCount(0)
  await expect(paper.getByRole('heading', { level: 2 })).toHaveText(QUESTION)
  await decisionsTaken(page, request)

  const snapshot = (await (await request.get('/api/orgs/acme/teams/app/view')).json()) as { view: { requests: Array<{ id: string }> } }
  expect(snapshot.view.requests.map((one) => one.id)).not.toContain(PROPOSAL_ID)

  // The request has left the team's view, but the log under the pile still names it, with where it stands.
  const recent = page.getByRole('region', { name: /Recently decided/ })
  await expect(recent.getByRole('listitem').first()).toContainText(PROPOSAL)
  await expect(recent.getByRole('listitem').first()).toContainText('Applied')
  const log = (await (await request.get('/api/decisions?limit=1')).json()) as { entries: Array<{ title?: string; target: string }> }
  expect(log.entries[0]).toMatchObject({ title: PROPOSAL, target: PROPOSAL_ID })
})

test('works from the keyboard: j and k move, a number picks, Control+Enter sends', async ({ page, request }) => {
  await enrolBrowser(page, request)
  await page.goto('/desk')
  await expect(page.getByRole('article').getByRole('heading', { level: 2 })).toHaveText(PROPOSAL)
  await page.keyboard.press('j')
  await expect(page.getByRole('article').getByRole('heading', { level: 2 })).toHaveText(QUESTION)
  await page.keyboard.press('k')
  await page.keyboard.press('2')
  await expect(page.getByRole('radio', { name: /Reject/ })).toBeChecked()
  await page.keyboard.press('Control+Enter')
  await decisionsTaken(page, request)
  const snapshot = (await (await request.get('/api/orgs/acme/teams/app/view')).json()) as { view: { requests: Array<{ id: string }> } }
  expect(snapshot.view.requests.map((one) => one.id)).not.toContain(PROPOSAL_ID)
})

test('asks for an answer before sending anything', async ({ page, request }) => {
  await enrolBrowser(page, request)
  await page.goto('/desk')
  await page.getByRole('article').getByRole('button', { name: 'Approve' }).click()
  await expect(page.getByText('Pick an option or write an answer first.')).toBeVisible()
  const snapshot = (await (await request.get('/api/orgs/acme/teams/app/view')).json()) as { view: { requests: unknown[] } }
  expect(snapshot.view.requests).toHaveLength(3)
})

test('reports a decision the node queued for later', async ({ page, request }) => {
  await resetNode(request, { outcome: 'queued' })
  await enrolBrowser(page, request)
  await page.goto('/desk')
  await page.getByRole('article').locator('label.choice').filter({ hasText: 'Approve' }).click()
  await page.getByRole('button', { name: 'Approve with “Approve”' }).click()
  // The node took it (queued): the tray is empty and the request does not come back while the log says it waits.
  await decisionsTaken(page, request)
  await expect(page.getByRole('listbox').getByText(PROPOSAL)).toHaveCount(0)
  await expect(page.getByRole('region', { name: /Recently decided/ }).getByRole('listitem').first()).toContainText('Saved, not delivered')
})

test('answers a pending decision in free text', async ({ page, request }) => {
  await enrolBrowser(page, request)
  await page.goto('/desk')
  await page.getByRole('option', { name: /Drop support for the old export format/ }).click()
  await page.getByRole('textbox', { name: 'Your answer' }).fill('Drop it in the next release.')
  await page.getByRole('button', { name: 'Send decision' }).click()
  // The request leaves the pile at once; the node has the decision once the tray is empty.
  await expect(page.getByRole('listbox').getByText(/Drop support for the old export format/)).toHaveCount(0)
  await decisionsTaken(page, request)
  expect(((await (await request.get('/__mock/envelopes')).json()) as unknown[]).length).toBe(1)
})

test('warns when a team’s data is a stale copy', async ({ page, request }) => {
  await resetNode(request, { siteError: 'connection refused' })
  await page.goto('/desk')
  await expect(page.getByText(/Showing a copy of this team from .*connection refused/)).toBeVisible()
})
