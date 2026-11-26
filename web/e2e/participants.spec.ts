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

test('shows nothing of the participants when the node gives them to no one', async ({ page, request }) => {
  await resetNode(request, { evaluation: true })
  await page.goto('/evaluation')
  await expect(page.getByRole('heading', { level: 2, name: 'App team' })).toBeVisible()
  await expect(page.getByRole('heading', { level: 4, name: 'backend' })).toHaveCount(0)
  await expect(page.getByRole('alert')).toHaveCount(0)
})

for (const [theme, width, height] of [['light', 1280, 900], ['dark', 390, 844]] as const) {
  test(`shows each participant’s time, cost, and merged pull requests in ${theme} at ${width}px, with no sideways scroll`, async ({ page, request }) => {
    mkdirSync(DIRECTORY, { recursive: true })
    await resetNode(request, { evaluation: true, participants: true })
    await page.setViewportSize({ width, height })
    await page.goto(`/evaluation?theme=${theme}`)
    const first = page.getByRole('heading', { level: 4, name: 'backend' }).first()
    await expect(first).toBeVisible()
    await first.scrollIntoViewIfNeeded()
    await expect(page.getByText('None merged').first()).toBeVisible()
    await page.locator('.ev-participants').first().screenshot({ path: `${DIRECTORY}/participants-${theme}-${width}.png`, animations: 'disabled' })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  })
}
