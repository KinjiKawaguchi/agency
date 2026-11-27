import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { decisionsTaken, enrolBrowser, resetNode, trackErrors } from './helpers'

const DIRECTORY = 'e2e-screenshots'
const PROPOSAL = 'Add a second frontend agent'
const QUESTION = 'Which database should the app use in production?'
const ENVELOPES = '**/api/orgs/*/teams/*/envelopes'
const TRAY = 'Sending decisions'

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  errors = trackErrors(page)
  await resetNode(request)
  await enrolBrowser(page, request)
})

async function approve(page: import('@playwright/test').Page): Promise<void> {
  await page.goto('/desk')
  const paper = page.getByRole('article')
  await paper.locator('label.choice').filter({ hasText: 'Approve' }).click()
  await paper.getByRole('button', { name: 'Approve with “Approve”' }).click()
}

test('moves to the next request at once while a slow node has not answered, and the decision still arrives', async ({ page, request }) => {
  let release: () => void = () => undefined
  const held = new Promise<void>((resolve) => {
    release = resolve
  })
  await page.route(ENVELOPES, async (route) => {
    await held
    await route.continue()
  })
  await approve(page)
  // The node has not answered: the next request is open at once, the decided one is off the pile, and the tray says it is on its way.
  await expect(page.getByRole('article').getByRole('heading', { level: 2 })).toHaveText(QUESTION, { timeout: 2_000 })
  await expect(page.getByRole('listbox').getByText(PROPOSAL)).toHaveCount(0)
  await expect(page.getByRole('region', { name: TRAY })).toContainText('Sending…')
  expect(((await (await request.get('/__mock/envelopes')).json()) as unknown[]).length).toBe(0)
  release()
  await decisionsTaken(page, request)
  expect(((await (await request.get('/__mock/envelopes')).json()) as unknown[]).length).toBe(1)
  await expect(page.getByRole('listbox').getByText(PROPOSAL)).toHaveCount(0)
  expect(errors()).toEqual([])
})

test('keeps a decision the node did not take, survives a reload, and sends the same one again by hand', async ({ page, request }) => {
  await page.route(ENVELOPES, (route) => route.abort('connectionrefused'))
  await approve(page)
  const tray = page.getByRole('region', { name: TRAY })
  await expect(tray).toContainText('Not sent yet', { timeout: 10_000 })
  // A reload loses nothing: the decision is kept in the browser and the request is still off the pile.
  await page.reload()
  await expect(tray).toBeVisible()
  await expect(page.getByRole('listbox').getByText(PROPOSAL)).toHaveCount(0)
  await page.unroute(ENVELOPES)
  await tray.getByRole('button', { name: `Send again: ${PROPOSAL}` }).click()
  await decisionsTaken(page, request)
  const accepted = (await (await request.get('/__mock/envelopes')).json()) as Array<{ body: { id: string } }>
  expect(accepted).toHaveLength(1)
})

for (const [theme, width, height] of [['light', 1280, 900], ['dark', 390, 844]] as const) {
  test(`shows a decision that failed in the tray, with its reason and what can be done, in ${theme} at ${width}px`, async ({ page }) => {
    mkdirSync(DIRECTORY, { recursive: true })
    await page.setViewportSize({ width, height })
    await page.route(ENVELOPES, (route) => route.fulfill({ status: 409, contentType: 'application/json', body: JSON.stringify({ error: 'the team’s node refused: That request is not open.' }) }))
    await page.goto(`/desk?theme=${theme}`)
    const paper = page.getByRole('article')
    await paper.locator('label.choice').filter({ hasText: 'Approve' }).click()
    await paper.getByRole('button', { name: 'Approve with “Approve”' }).click()
    const tray = page.getByRole('region', { name: TRAY })
    await expect(tray).toContainText('Not sent: the team’s node refused: That request is not open.')
    await expect(tray.getByRole('button', { name: `Send again: ${PROPOSAL}` })).toBeVisible()
    await expect(tray.getByRole('button', { name: `Discard: ${PROPOSAL}` })).toBeVisible()
    await page.screenshot({ path: `${DIRECTORY}/tray-failed-${theme}-${width}.png`, animations: 'disabled' })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
    // Discarding gives the request back to the pile.
    await tray.getByRole('button', { name: `Discard: ${PROPOSAL}` }).click()
    await expect(page.getByRole('listbox').getByText(PROPOSAL)).toHaveCount(1)
  })
}
