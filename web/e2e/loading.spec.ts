import { mkdirSync } from 'node:fs'
import { expect, test, type Page } from '@playwright/test'
import { pageScrollsSideways, resetNode, stickingOut, waitOnClock } from './helpers'

// A team's view that is not ready yet (the node answers 503) is loading: a skeleton in place, tried again with
// backoff, an error only when it keeps failing. The timers of the page are driven by hand, so the backoff takes no time.
const DIRECTORY = 'e2e-screenshots'
const VIEW = '**/api/orgs/*/teams/*/view'

test.beforeAll(() => mkdirSync(DIRECTORY, { recursive: true }))
test.beforeEach(async ({ request }) => {
  await resetNode(request, { moreOrgs: false })
})

/** The node answers 503 to every read of a team's view, or to the first `times`. */
async function notReady(page: Page, times = Number.POSITIVE_INFINITY): Promise<{ reads: () => number }> {
  let reads = 0
  await page.route(VIEW, async (route) => {
    reads += 1
    if (reads <= times) return route.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ error: 'The board has not been read yet; a refresh was started.' }) })
    return route.fallback()
  })
  return { reads: () => reads }
}

const SCREENS = [
  { name: 'overview', path: '/overview' },
  { name: 'org', path: '/org' },
  { name: 'people', path: '/people' },
] as const

for (const screen of SCREENS) {
  test(`${screen.name}: a skeleton while the view is not ready, with no error and nothing sideways`, async ({ page }) => {
    await page.clock.install()
    await notReady(page)
    await page.setViewportSize({ width: 375, height: 800 })
    await page.goto(screen.path)
    await expect(page.locator('[aria-busy="true"]').first()).toBeAttached()
    await page.clock.runFor(1500)
    // Tried again after the first wait, and still loading: no failure is shown.
    await expect(page.locator('.skeleton').first()).toBeVisible()
    await expect(page.getByRole('alert')).toHaveCount(0)
    await expect(page.getByText(/Could not load/)).toHaveCount(0)
    expect(await pageScrollsSideways(page)).toBe(false)
    expect(await stickingOut(page)).toEqual([])
  })

  test(`${screen.name}: draws the screen once the view is ready, without ever showing an error`, async ({ page }) => {
    await page.clock.install()
    const { reads } = await notReady(page, 4)
    await page.goto(screen.path)
    await page.clock.runFor(1_000)
    await expect(page.locator('.region-failed, .ov-failed')).toHaveCount(0)
    await page.clock.runFor(10_000)
    await expect(page.locator('.skeleton')).toHaveCount(0)
    await expect(page.locator('.region-failed')).toHaveCount(0)
    expect(reads()).toBeGreaterThan(4)
  })
}

for (const theme of ['light', 'dark'] as const) {
  for (const viewport of [{ name: 'desktop', width: 1440, height: 900 }, { name: 'phone', width: 390, height: 844 }] as const) {
    for (const screen of SCREENS) {
      test(`screenshot of the ${screen.name} skeleton, ${theme}, ${viewport.name}`, async ({ page }) => {
        await page.clock.install()
        await notReady(page)
        await page.setViewportSize({ width: viewport.width, height: viewport.height })
        await page.goto(`${screen.path}?theme=${theme}`)
        await expect(page.locator('.skeleton').first()).toBeVisible()
        await page.screenshot({ path: `${DIRECTORY}/loading-${screen.name}-${theme}-${viewport.name}.png`, animations: 'disabled' })
      })
    }
  }
}

test('a team that keeps failing is an error in its place, with why and when, and the retry button works', async ({ page }) => {
  await page.clock.install()
  let isReady = false
  await page.route(VIEW, (route) => (isReady ? route.fallback() : route.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ error: 'The runtime did not answer within 20 seconds.' }) })))
  await page.goto('/overview')
  await waitOnClock(page, async () => (await page.locator('.ov-failed-block').count()) > 0)
  const failed = page.locator('.ov-failed-block').first()
  await expect(failed).toContainText('Could not load this team: The runtime did not answer within 20 seconds.')
  await expect(failed).toContainText(/Trying again in \d+ s\./)
  await page.screenshot({ path: `${DIRECTORY}/loading-failed-overview-light-desktop.png`, animations: 'disabled' })
  isReady = true
  await failed.getByRole('button', { name: 'Try again' }).click()
  await waitOnClock(page, async () => (await page.locator('.ov-failed-block').count()) === 0)
})

test('the org chart that keeps failing says so in its place and the menu stays', async ({ page }) => {
  await page.clock.install()
  await page.route('**/api/orgs/acme', (route) => route.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ error: 'The org’s node did not answer.' }) }))
  await page.goto('/org')
  await waitOnClock(page, async () => (await page.getByRole('alert').count()) > 0)
  const alert = page.getByRole('alert')
  await expect(alert).toContainText('Could not load the org chart: The org’s node did not answer.')
  await expect(page.getByRole('link', { name: 'Overview' })).toBeVisible()
  await page.screenshot({ path: `${DIRECTORY}/loading-failed-org-light-desktop.png`, animations: 'disabled' })
})
