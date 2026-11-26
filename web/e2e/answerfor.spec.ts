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
  await resetNode(request, { relayedFor: true })
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

for (const theme of ['light', 'dark'] as const) {
  for (const viewport of VIEWPORTS) {
    test(`a request for a seat the viewer answers for says whom it is for, in ${theme} at ${viewport.name} width`, async ({ page }) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height })
      await page.goto(`/desk?theme=${theme}`)
      await page.getByRole('option', { name: /Add a second frontend agent/ }).click()
      await expect(page.getByRole('option', { name: /Add a second frontend agent/ }).getByText('Answer for Erin (outside counsel)')).toBeVisible()
      await expect(page.getByRole('heading', { level: 2, name: 'Add a second frontend agent' }).locator('..').getByText('Answer for Erin (outside counsel)')).toBeVisible()
      expect(await pageScrollsSideways(page)).toBe(false)
      await page.screenshot({ path: `${DIRECTORY}/answerfor-${theme}-${viewport.name}.png`, animations: 'disabled' })
    })
  }
}

