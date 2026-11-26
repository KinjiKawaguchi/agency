import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { pageScrollsSideways, resetNode, trackErrors } from './helpers'

const DIRECTORY = 'e2e-screenshots'
const VIEWPORTS = [
  { name: 'desktop', width: 1280, height: 900 },
  { name: 'phone', width: 390, height: 844 },
] as const

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  mkdirSync(DIRECTORY, { recursive: true })
  await resetNode(request, { guest: 'carol' })
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

for (const theme of ['light', 'dark'] as const) {
  for (const viewport of VIEWPORTS) {
    test(`a member the node hosts has their decisions and their devices, in ${theme} at ${viewport.name} width`, async ({ page }) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height })
      await page.goto(`/desk?theme=${theme}`)
      const menu = page.getByRole('navigation', { name: 'Main menu' })
      await expect(menu.getByRole('link')).toHaveCount(2)
      await expect(menu.getByRole('link', { name: /^Decisions/ })).toBeVisible()
      await expect(menu.getByRole('link', { name: /^Devices/ })).toBeVisible()
      await expect(page.getByText('Add a second frontend agent').first()).toBeVisible()
      expect(await pageScrollsSideways(page)).toBe(false)
      await page.screenshot({ path: `${DIRECTORY}/guest-desk-${theme}-${viewport.name}.png`, animations: 'disabled' })
      await menu.getByRole('link', { name: /^Devices/ }).click()
      await expect(page.getByRole('heading', { level: 1, name: 'Devices' })).toBeVisible()
      await expect(page.getByRole('heading', { name: /Home nodes/ })).toHaveCount(0)
      expect(await pageScrollsSideways(page)).toBe(false)
      await page.screenshot({ path: `${DIRECTORY}/guest-devices-${theme}-${viewport.name}.png`, animations: 'disabled' })
    })
  }
}

test('a page of the owner’s is not opened for a guest, and none of the owner’s reads is made', async ({ page }) => {
  const refused: string[] = []
  page.on('response', (response) => {
    if (response.status() === 403) refused.push(new URL(response.url()).pathname)
  })
  await page.goto('/costs')
  await expect(page).toHaveURL(/\/desk$/)
  await expect(page.getByText('Add a second frontend agent').first()).toBeVisible()
  expect(refused).toEqual([])
})
