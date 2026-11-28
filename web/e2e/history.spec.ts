import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { pageScrollsSideways, resetNode, stickingOut, trackErrors } from './helpers'

const DIRECTORY = 'e2e-screenshots'
const VIEWPORTS = [
  { name: 'desktop', width: 1280, height: 900 },
  { name: 'phone', width: 390, height: 844 },
] as const

let errors: () => string[]

test.beforeEach(({ page }) => {
  mkdirSync(DIRECTORY, { recursive: true })
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

for (const theme of ['light', 'dark'] as const) {
  for (const viewport of VIEWPORTS) {
    test(`shows the owner an org’s history, with a gap, a version compared across it and a signature not checked, in ${theme} at ${viewport.name} width`, async ({ page, request }) => {
      await resetNode(request, { history: 3 })
      await page.setViewportSize({ width: viewport.width, height: viewport.height })
      await page.goto(`/history?theme=${theme}`)
      await expect(page.getByRole('heading', { level: 1, name: 'History' })).toBeVisible()
      await expect(page.getByText('This node does not keep this version.')).toBeVisible()
      await expect(page.getByText('add member bob (Bob)')).toBeVisible()
      expect(await pageScrollsSideways(page)).toBe(false)
      expect(await stickingOut(page)).toEqual([])
      await page.screenshot({ path: `${DIRECTORY}/history-${theme}-${viewport.name}.png`, fullPage: true, animations: 'disabled' })
    })
  }
}

test('pages a long history: the first page, then the later versions on a click', async ({ page, request }) => {
  await resetNode(request, { history: 230 })
  await page.goto('/history')
  await expect(page.getByText('Version 200', { exact: true })).toBeVisible()
  await expect(page.getByText('Version 201', { exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: 'Show later versions' }).click()
  await expect(page.getByText('Version 230', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Show later versions' })).toHaveCount(0)
})

test('has no entry in the menu and says so on a node that does not report the history', async ({ page, request }) => {
  await resetNode(request, {})
  await page.goto('/history')
  await expect(page.getByText('This node does not report the history yet.')).toBeVisible()
  await expect(page.getByRole('link', { name: 'History' })).toHaveCount(0)
})
