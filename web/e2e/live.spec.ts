import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { resetNode, trackErrors } from './helpers'

// The live page: a picture of who writes to whom, a feed, what each agent is on, and the throughput, kept up to
// date while it is open. It runs against the mock node, whose feed goes on as the clock does.
const DIRECTORY = 'e2e-screenshots'

test.beforeAll(() => mkdirSync(DIRECTORY, { recursive: true }))
test.beforeEach(async ({ request }) => {
  await resetNode(request, { activity: true, evaluation: true, moreOrgs: false })
})

test('shows the agents, their messages, the feed, and what they work on, with no error', async ({ page }) => {
  const errors = trackErrors(page)
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/live')
  await expect(page.locator('.lv-node').first()).toBeVisible()
  await expect(page.locator('line.lv-edge').first()).toBeVisible()
  await expect(page.locator('.lv-feed .lv-row').first()).toBeVisible()
  await expect(page.getByRole('region', { name: 'Working on now' }).getByText('app#12').first()).toBeVisible()
  expect(errors()).toEqual([])
})

test('takes the picture in light and dark, at desktop and phone width, and a message passing along its line', async ({ page }) => {
  test.setTimeout(120_000)
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/live?theme=dark')
  await expect(page.locator('line.lv-edge').first()).toBeVisible()
  await page.screenshot({ path: `${DIRECTORY}/live-dark-desktop.png`, animations: 'disabled' })
  // The page reads the feed again every 10 seconds; when something new has come, a packet travels along its line.
  const packet = page.locator('.lv-packet').first()
  // A message passes about every ten seconds for each team in the mock, so a packet is in flight now and then: wait for one.
  await expect(packet).toBeVisible({ timeout: 70_000 })
  await page.screenshot({ path: `${DIRECTORY}/live-dark-desktop-packet.png`, animations: 'disabled' })
  await page.goto('/live?theme=light')
  await expect(page.locator('line.lv-edge').first()).toBeVisible()
  await page.screenshot({ path: `${DIRECTORY}/live-light-desktop.png`, animations: 'disabled' })
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto('/live?theme=dark')
  await expect(page.locator('line.lv-edge').first()).toBeVisible()
  await page.screenshot({ path: `${DIRECTORY}/live-dark-phone.png`, fullPage: true, animations: 'disabled' })
})

test('moves nothing when less motion is asked for', async ({ browser }) => {
  const context = await browser.newContext({ reducedMotion: 'reduce', viewport: { width: 1200, height: 800 } })
  const page = await context.newPage()
  await page.goto('/live')
  await expect(page.locator('line.lv-edge').first()).toBeVisible()
  await page.waitForTimeout(12_000)
  await expect(page.locator('.lv-packet')).toHaveCount(0)
  const before = await page.locator('.lv-node').first().getAttribute('transform')
  await page.waitForTimeout(1_500)
  expect(await page.locator('.lv-node').first().getAttribute('transform')).toBe(before)
  await context.close()
})
