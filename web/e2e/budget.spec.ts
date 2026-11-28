import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { pageScrollsSideways, resetNode, trackErrors } from './helpers'

const DIRECTORY = 'e2e-screenshots'
const VIEWPORTS = [
  { name: 'desktop', width: 1280, height: 900 },
  { name: 'phone', width: 390, height: 844 },
] as const

/** Where the `app` team stands against its weekly budget, as the mock node says it, and what the meter says of each. */
const STATES = [
  { name: 'ok', says: 'Within the budget' },
  { name: 'warning', says: 'Close to the budget' },
  { name: 'over', says: 'Over the budget' },
  { name: 'partial', says: 'This team’s usage could not be read, so what it spent this week is not known.' },
] as const

let errors: () => string[]

test.beforeEach(({ page }) => {
  mkdirSync(DIRECTORY, { recursive: true })
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

for (const state of STATES) {
  for (const theme of ['light', 'dark'] as const) {
    for (const viewport of VIEWPORTS) {
      test(`draws a team ${state.name} its weekly budget in ${theme} at ${viewport.name} width`, async ({ page, request }) => {
        await resetNode(request, { evaluation: true, budgetState: state.name })
        await page.setViewportSize({ width: viewport.width, height: viewport.height })
        await page.goto(`/evaluation?theme=${theme}`)
        const meter = page.locator('.ev-budget').first()
        await expect(meter).toContainText(state.says)
        expect(await pageScrollsSideways(page)).toBe(false)
        // The page is scrolled so that the meter is clear of the fixed header, and the screenshot is of the page's visible area around it.
        await meter.evaluate((element) => element.scrollIntoView({ block: 'center' }))
        await page.screenshot({ path: `${DIRECTORY}/budget-${state.name}-${theme}-${viewport.name}.png`, animations: 'disabled' })
      })
    }
  }
}
