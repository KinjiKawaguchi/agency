import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { pageScrollsSideways, resetNode, trackErrors } from './helpers'

// The flows a first-time person goes through on a phone, at the width of one.
const DIRECTORY = 'e2e-screenshots'
const PHONE = { width: 390, height: 844 }

test.use({ viewport: PHONE })

let errors: () => string[]

test.beforeAll(() => mkdirSync(DIRECTORY, { recursive: true }))

test.beforeEach(({ page }) => {
  errors = trackErrors(page)
})

test.afterEach(async ({ page }) => {
  expect(await pageScrollsSideways(page)).toBe(false)
  expect(errors()).toEqual([])
})

test('the first screen without a device leads to adding it, and the document opens above the old decisions', async ({ page, request }) => {
  await resetNode(request)
  await page.goto('/desk')
  const notice = page.locator('.device-notice')
  await expect(notice).toContainText('Add this device so your decisions are signed with a key that never leaves it.')
  await expect(notice.getByRole('link', { name: 'Add this device' })).toBeVisible()

  const paper = await page.getByRole('article').boundingBox()
  const recent = await page.getByRole('region', { name: /Recently decided/ }).boundingBox()
  expect(paper?.y).toBeLessThan(recent?.y ?? 0)

  await notice.getByRole('link', { name: 'Add this device' }).click()
  await expect(page.getByRole('heading', { level: 1, name: 'Devices' })).toBeVisible()
  await expect(page.locator('.device-notice')).toHaveCount(0)
})

test('opening a document from the pile brings it into view', async ({ page, request }) => {
  await resetNode(request)
  await page.goto('/desk')
  await expect(page.getByRole('article')).toBeVisible()
  await page.getByRole('option', { name: /Which database should the app use in production/ }).click()
  await expect(page.getByRole('article').getByRole('heading', { level: 2 })).toHaveText('Which database should the app use in production?')
  await expect.poll(async () => (await page.getByRole('article').boundingBox())?.y ?? 9999, { timeout: 5000 }).toBeLessThan(200)
})

test('an empty desk says nothing waits, with no blank paper', async ({ page, request }) => {
  await resetNode(request, { empty: true })
  await page.goto('/desk')
  await expect(page.getByText('Nothing is waiting for you. New requests from your team show up here.')).toBeVisible()
  await expect(page.getByRole('article')).toHaveCount(0)
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Nothing needs your decision right now.')
})

for (const theme of ['light', 'dark'] as const) {
  test(`phone screenshots of the first run in ${theme}`, async ({ page, request }) => {
    await resetNode(request)
    const shoot = (name: string) => page.screenshot({ path: `${DIRECTORY}/phone-${name}-${theme}.png`, fullPage: true, animations: 'disabled' })

    await page.goto(`/desk?theme=${theme}`)
    await expect(page.locator('.device-notice')).toBeVisible()
    await shoot('first-screen')

    await page.locator('.device-notice').getByRole('link', { name: 'Add this device' }).click()
    await expect(page.getByRole('button', { name: 'Add this device' })).toBeVisible()
    await shoot('add-device')

    await page.getByRole('button', { name: 'Add this device' }).click()
    await expect(page.getByText(/Waiting for you to confirm this device/)).toBeVisible()
    await shoot('waiting')

    await resetNode(request, { empty: true })
    await page.goto('/desk')
    await expect(page.getByText('Nothing is waiting for you. New requests from your team show up here.')).toBeVisible()
    await shoot('empty-desk')
  })
}
