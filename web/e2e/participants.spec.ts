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

test.describe('in Japanese', () => {
  test.use({ locale: 'ja-JP' })
  for (const theme of ['light', 'dark'] as const) {
    test(`shows the participants card in ${theme} at 375px, with no sideways scroll and no word cut off`, async ({ page, request }) => {
      mkdirSync(DIRECTORY, { recursive: true })
      await resetNode(request, { evaluation: true, participants: true })
      await page.setViewportSize({ width: 375, height: 812 })
      await page.goto(`/evaluation?theme=${theme}`)
      const card = page.locator('.ev-participants').first()
      await expect(card.getByText('メンバー', { exact: true })).toBeVisible()
      await expect(card.getByText('マージなし').first()).toBeVisible()
      await expect(card.getByText('相談待ち（下限）').first()).toBeVisible()
      await card.scrollIntoViewIfNeeded()
      await card.screenshot({ path: `${DIRECTORY}/participants-ja-${theme}-375.png`, animations: 'disabled' })
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
      // Nothing inside the card is wider than the card: a long word that does not break would push out of it.
      expect(await card.evaluate((element) => Array.from(element.querySelectorAll('*')).filter((child) => child.getBoundingClientRect().right > element.getBoundingClientRect().right + 1).length)).toBe(0)
    })
  }
})
