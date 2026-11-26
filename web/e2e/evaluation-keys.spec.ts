import { expect, test } from '@playwright/test'
import { resetNode } from './helpers'

// A chart of days is one tab stop, and the arrow keys walk its days; the day in focus tells its numbers.
test('walks the days of a chart with the arrow keys, and each day tells its numbers', async ({ page, request }) => {
  await resetNode(request, { evaluation: true })
  await page.setViewportSize({ width: 1200, height: 900 })
  await page.goto('/evaluation')
  const chart = page.getByRole('group', { name: 'Merged pull requests per day' }).first()
  await expect(chart.locator('.ev-col').first()).toBeVisible()
  expect(await chart.locator('.ev-col[tabindex="0"]').count()).toBe(1)
  await chart.locator('.ev-col[tabindex="0"]').focus()
  const newest = chart.locator('.ev-col').last()
  await expect(newest).toBeFocused()
  await page.keyboard.press('ArrowLeft')
  await expect(chart.locator('.ev-col').nth(-2)).toBeFocused()
  await expect(chart.locator('.ev-col').nth(-2).locator('.ev-tip')).toBeVisible()
  await page.keyboard.press('Home')
  await expect(chart.locator('.ev-col').first()).toBeFocused()
  await expect(chart.locator('.ev-col').first().locator('.ev-tip')).toBeVisible()
  // Tabbing on leaves the chart: the days are not 14 stops.
  await page.keyboard.press('Tab')
  expect(await chart.locator('.ev-col').evaluateAll((days) => days.filter((day) => day === document.activeElement).length)).toBe(0)
})
