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
    await expect(page.getByRole('article', { name: 'Monthly revenue' }).getByText('Cost on the same days')).toBeVisible()
    await expect(page.getByText('Too early: a change needs 14 days to show. It can be read from Dec 4.')).toBeVisible()
    await page.screenshot({ path: `${DIRECTORY}/outcomes-${theme}-${width}.png`, fullPage: true, animations: 'disabled' })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  })
}

for (const [theme, width, height] of [['light', 1280, 900], ['dark', 390, 844]] as const) {
  test(`enters a number on a KPI's card and reads its history in ${theme} at ${width}px`, async ({ page, request }) => {
    mkdirSync(DIRECTORY, { recursive: true })
    await resetNode(request, { outcomes: true })
    await page.setViewportSize({ width, height })
    await page.goto(`/outcomes?theme=${theme}`)
    const revenue = page.getByRole('article', { name: 'Monthly revenue' })
    await revenue.getByRole('button', { name: 'Enter a number' }).click()
    const form = page.getByRole('form', { name: 'Enter a number: Monthly revenue' })
    await form.getByLabel('Number').fill('abc')
    await form.getByRole('button', { name: 'Save' }).click()
    await expect(form.getByRole('alert')).toContainText('The number must be written with digits')
    await form.getByLabel('Number').fill('1400000')
    await form.getByLabel('Note (optional)').fill('Month closed')
    await page.screenshot({ path: `${DIRECTORY}/outcomes-enter-${theme}-${width}.png`, fullPage: true, animations: 'disabled' })
    await form.getByRole('button', { name: 'Save' }).click()
    await expect(revenue.locator('.oc-value')).toHaveText('¥1,400,000')
    await revenue.getByRole('button', { name: 'Show history' }).click()
    const history = revenue.getByRole('list', { name: 'History: Monthly revenue' })
    await expect(history.getByRole('listitem')).toHaveCount(3)
    await expect(history.getByRole('listitem').first()).toContainText('Month closed')
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  })
}
