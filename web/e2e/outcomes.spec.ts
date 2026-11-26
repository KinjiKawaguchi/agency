import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { resetNode, trackErrors } from './helpers'

const DIRECTORY = 'e2e-screenshots'

let errors: () => string[]

test.beforeEach(({ page }) => {
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

test('for anyone but the owner there is no page, no menu entry, and no complaint', async ({ page, request }) => {
  await resetNode(request)
  await page.goto('/outcomes')
  await expect(page.getByRole('heading', { level: 1, name: 'Overview' })).toBeVisible()
  await page.goto('/costs')
  await expect(page.getByRole('navigation', { name: 'Results pages' })).toBeVisible()
  await expect(page.getByRole('link', { name: 'Outcomes' })).toHaveCount(0)
})

for (const [theme, width, height] of [['light', 1280, 900], ['dark', 390, 844]] as const) {
  test(`draws the owner’s KPIs and readings in ${theme} at ${width}px, with no sideways scroll`, async ({ page, request }) => {
    mkdirSync(DIRECTORY, { recursive: true })
    await resetNode(request, { outcomes: true })
    await page.setViewportSize({ width, height })
    await page.goto(`/outcomes?theme=${theme}`)
    await expect(page.getByRole('heading', { level: 1, name: 'Outcomes' })).toBeVisible()
    await expect(page.getByRole('article', { name: 'Monthly revenue' })).toBeVisible()
    await expect(page.getByRole('heading', { level: 2, name: 'Readings' })).toBeVisible()
    await expect(page.getByText('Too early: a change needs 14 days to show. It can be read from Dec 4.')).toBeVisible()
    await page.screenshot({ path: `${DIRECTORY}/outcomes-${theme}-${width}.png`, fullPage: true, animations: 'disabled' })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  })
}
