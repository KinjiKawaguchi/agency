import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { resetNode, trackErrors } from './helpers'

// What the person is told about this browser's key, in a real browser: that it may be removed, and that an earlier one is gone.

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  await resetNode(request)
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

const CODE = /^[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}$/
const KEPT = /keeps this device’s key until you clear its site data/
const AT_RISK = /may remove this device’s key if you clear its site data/
const LOST = /earlier key is gone/

test('says whether the browser keeps the key, and asks it to when the person asks to add this device', async ({ page }) => {
  await page.goto('/devices')
  await expect(page.getByLabel('Pairing code of this device')).toHaveText(CODE)
  await expect(page.getByText(KEPT).or(page.getByText(AT_RISK))).toBeVisible()
  await expect(page.getByText(LOST)).toHaveCount(0)
  await page.getByRole('button', { name: 'Add this device' }).click()
  await expect(page.getByText(/Waiting for you to approve this device/)).toBeVisible()
  await expect(page.getByText(KEPT).or(page.getByText(AT_RISK))).toBeVisible()
})

for (const view of [
  { name: 'desktop', width: 1440, height: 900, theme: 'light' },
  { name: 'phone', width: 390, height: 844, theme: 'dark' },
] as const) {
  test(`says an earlier key is gone, in ${view.theme} at ${view.name} width, when the browser lost the key but not its other data`, async ({ page }) => {
    mkdirSync('e2e-screenshots', { recursive: true })
    await page.setViewportSize({ width: view.width, height: view.height })
    await page.goto(`/devices?theme=${view.theme}`)
    const first = (await page.getByLabel('Pairing code of this device').textContent()) ?? ''
    expect(first).toMatch(CODE)
    await expect(page.getByText(LOST)).toHaveCount(0)
    // The key's database is removed and the rest of the site's data stays, as when a browser drops one store.
    await page.evaluate(
      () =>
        new Promise<void>((resolve, reject) => {
          const deleting = indexedDB.deleteDatabase('agency-device')
          deleting.onsuccess = () => resolve()
          deleting.onerror = () => reject(deleting.error)
          deleting.onblocked = () => resolve()
        }),
    )
    await page.reload()
    await expect(page.getByLabel('Pairing code of this device')).toHaveText(CODE)
    expect(await page.getByLabel('Pairing code of this device').textContent()).not.toBe(first)
    await expect(page.getByRole('alert').filter({ hasText: 'earlier key is gone' })).toBeVisible()
    await page.screenshot({ path: `e2e-screenshots/key-lost-${view.theme}-${view.name}.png`, fullPage: true, animations: 'disabled' })
  })
}
