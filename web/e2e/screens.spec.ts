import { expect, test } from '@playwright/test'
import { resetNode, trackErrors, waitOnClock } from './helpers'

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  await resetNode(request)
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

test('overview: every org and team, what needs the viewer first, and a team that cannot be read in its place', async ({ page, request }) => {
  await resetNode(request, { moreOrgs: true, usage: true })
  // The team nobody can read is tried again with backoff before it is called failed; the page's clock is moved on by hand.
  await page.clock.install()
  await page.goto('/overview')
  await expect(page.getByRole('heading', { level: 1, name: 'Overview' })).toBeVisible()
  await expect(page.locator('article.ov-team')).toHaveCount(6)
  await waitOnClock(page, async () => (await page.getByRole('article', { name: 'club/shop' }).getByText('Could not load this team').count()) > 0)
  await expect(page.locator('section.ov-org h2').allTextContents()).resolves.toEqual(['Acme', 'Lab', 'Club', 'Usage limits'])
  const research = page.getByRole('article', { name: 'lab/research' })
  await expect(research).toContainText('3 blocked items')
  await expect(page.getByRole('article', { name: 'lab/notebook' })).toContainText('The team’s node could not be reached')
  await expect(page.getByRole('article', { name: 'club/shop' })).toContainText('Could not load this team')
  await page.getByRole('link', { name: '1 request waits for you' }).click()
  await expect(page).toHaveURL(/\/desk\?team=lab/)
  await expect(page.getByText('Showing only Notebook.')).toBeVisible()
  await expect(page.getByRole('article').getByRole('heading', { level: 2 })).toHaveText('Publish the notebook under the lab’s name?')
})

test('people and work: who waits on you, work in progress, blocked work, and epic forecasts', async ({ page }) => {
  await page.goto('/people')
  await expect(page.getByRole('heading', { level: 2, name: 'App team' })).toBeVisible()
  const backend = page.locator('article.mate', { hasText: 'Permission prompt' })
  await expect(backend).toContainText('Your turn')
  await expect(backend).toContainText('Waiting for your answer: “Which database should the app use in production?”')
  await expect(page.locator('.work-row', { hasText: 'Store sessions in the database' })).toContainText('Expected around')
  await expect(page.locator('.work-row', { hasText: 'Remove the old export format' })).toContainText('Needs a decision since Nov 23')
  await expect(page.getByRole('img', { name: '1 of 4 done, 1 in progress, 2 blocked' })).toBeVisible()
  await backend.getByRole('link', { name: 'Open decisions' }).click()
  await expect(page).toHaveURL(/\/desk/)
})

test('org chart: departments, teams, agents, and focusing on one department', async ({ page }) => {
  await page.goto('/org')
  const chart = page.getByRole('group', { name: 'Organization chart' })
  await expect(chart.getByText('Acme', { exact: true })).toBeVisible()
  const engineering = chart.getByRole('link', { name: /DepartmentEngineering|Department\s*Engineering/ })
  await expect(engineering).toContainText('Led by Alice')
  await expect(engineering).toContainText('3 requests wait for you')
  await expect(chart.getByRole('link', { name: /Backend/ })).toContainText('Waiting for your answer')
  await engineering.click()
  await expect(page).toHaveURL(/unit=engineering/)
  await expect(page.getByRole('link', { name: 'Back to the whole organization' })).toBeVisible()
  await expect(chart.getByText('Design', { exact: true })).toHaveCount(0)
  await page.getByRole('link', { name: 'Back to the whole organization' }).click()
  await expect(chart.getByRole('link', { name: /Design/ }).first()).toBeVisible()
})

test('org chart: units under one parent sit side by side, and on a phone the same tree is an indented list', async ({ page }) => {
  const box = async (text: string) => {
    const found = await page.locator('.oc-node', { hasText: text }).first().boundingBox()
    if (found === null) throw new Error(`no box for ${text}`)
    return found
  }
  const agents = ['Reviewing app#12', 'Waiting for your answer', 'Idle for 30 minutes', 'Session ended']

  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/org')
  await expect(page.locator('.orgchart .oc-node.agent').first()).toBeVisible()
  const wide = await Promise.all(agents.map(box))
  const team = await box('4 agents, 2 working')
  expect(new Set(wide.map((one) => Math.round(one.y))).size).toBe(1)
  expect(wide.map((one) => one.x)).toEqual(wide.map((one) => one.x).toSorted((a, b) => a - b))
  expect(wide[0]?.y).toBeGreaterThan(team.y + team.height)
  // The sibling departments and teams are in one row too.
  const [engineering, design] = [await box('Led by Alice'), await box('Led by Carol')]
  expect(Math.round(engineering.y)).toBe(Math.round(design.y))

  await page.setViewportSize({ width: 375, height: 800 })
  await expect(page.locator('.orgchart .oc-node.agent').first()).toBeVisible()
  const narrow = await Promise.all(agents.map(box))
  const narrowTeam = await box('4 agents, 2 working')
  expect(new Set(narrow.map((one) => Math.round(one.x))).size).toBe(1)
  expect(narrow.map((one) => one.y)).toEqual(narrow.map((one) => one.y).toSorted((a, b) => a - b))
  expect(narrow[0]?.x).toBeGreaterThan(narrowTeam.x)
  expect(await page.locator('.oc-frame').evaluate((frame) => frame.scrollWidth <= frame.clientWidth)).toBe(true)
})

test('costs: subscription windows are projected, and token use says the team does not report it', async ({ page }) => {
  await page.goto('/costs')
  const personal = page.locator('article.fund', { hasText: 'Personal plan' })
  await expect(personal.getByRole('img', { name: /Weekly window: 52% used, about 9\d% at the reset/ })).toBeVisible()
  await expect(personal.getByText('Will run out')).toBeVisible()
  await expect(page.locator('article.fund', { hasText: 'Lab plan' }).getByText('Will go unused')).toBeVisible()
  await expect(page.getByText('This team does not report token usage.')).toHaveCount(2)
})

test('costs: token use by day and by person when the node serves it', async ({ page, request }) => {
  await resetNode(request, { usage: true })
  await page.goto('/costs')
  const columns = page.getByRole('group', { name: 'Cost by day' }).first().locator('.column')
  await expect(columns).toHaveCount(14)
  await page.getByRole('button', { name: '30 days' }).first().click()
  await expect(columns).toHaveCount(30)
  await columns.nth(3).hover()
  await expect(columns.nth(3).locator('.tip')).toBeVisible()
  await expect(page.getByRole('table').first().getByRole('rowheader')).toHaveCount(4)
})

test('switches the language and keeps it across a reload', async ({ page }) => {
  await page.goto('/desk')
  await page.getByRole('combobox', { name: 'Language' }).selectOption('ja')
  await expect(page.getByRole('heading', { level: 1 })).toContainText('参加者 1 人があなたの返事を待っています。')
  await expect(page.locator('html')).toHaveAttribute('lang', 'ja')
  await page.reload()
  await expect(page.getByRole('heading', { level: 1 })).toContainText('参加者 1 人があなたの返事を待っています。')
})

test('follows the theme the viewer picks', async ({ page }) => {
  await page.goto('/desk')
  await page.getByRole('combobox', { name: 'Theme' }).selectOption('dark')
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  const background = await page.evaluate(() => getComputedStyle(document.body).backgroundColor)
  expect(background).toBe('rgb(12, 13, 16)')
})
