import { readFileSync } from 'node:fs'
import { expect, test, type Page } from '@playwright/test'
import { resetNode } from './helpers'

// The org chart takes the window's width and can be downloaded as an image of the whole chart, in the
// theme the page is in: not only the part in view, and not on a white page when the page is dark.

const THEMES = ['light', 'dark'] as const

/** The pixel size a PNG says it has (its IHDR chunk). */
function pngSize(bytes: Buffer): { width: number; height: number } {
  expect(bytes.subarray(0, 8).toString('hex')).toBe('89504e470d0a1a0a')
  return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) }
}

/** The color of the top-left pixel of a PNG and how many pixels differ from it, read in the page. */
function inspectPng(page: Page, bytes: Buffer): Promise<{ corner: number[]; different: number }> {
  return page.evaluate(async (base64) => {
    const image = new Image()
    await new Promise<void>((resolve, reject) => {
      image.onload = () => resolve()
      image.onerror = () => reject(new Error('not an image'))
      image.src = `data:image/png;base64,${base64}`
    })
    const canvas = document.createElement('canvas')
    canvas.width = image.width
    canvas.height = image.height
    const context = canvas.getContext('2d')!
    context.drawImage(image, 0, 0)
    const { data } = context.getImageData(0, 0, canvas.width, canvas.height)
    const corner = [data[0] ?? 0, data[1] ?? 0, data[2] ?? 0]
    let different = 0
    for (let at = 0; at < data.length; at += 4) if (data[at] !== corner[0] || data[at + 1] !== corner[1] || data[at + 2] !== corner[2]) different += 1
    return { corner, different }
  }, bytes.toString('base64'))
}

test.beforeEach(async ({ request }) => {
  await resetNode(request, { moreOrgs: true })
})

test('the chart takes the window’s width, not the text column', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 })
  await page.goto('/org')
  const frame = page.locator('.oc-frame')
  await expect(frame.locator('.oc-node.team').first()).toBeVisible()
  const width = (await frame.boundingBox())?.width ?? 0
  // The page keeps its gutters (24px a side), and nothing else.
  expect(width).toBeGreaterThan(1600 - 2 * 24 - 2)
})

for (const theme of THEMES) {
  test(`downloads the whole chart as a PNG in the ${theme} theme`, async ({ page }) => {
    await page.setViewportSize({ width: 1200, height: 800 })
    await page.goto(`/org?theme=${theme}`)
    await expect(page.locator('.orgchart.all .oc-node.team').first()).toBeVisible()
    // The chart at 100% is wider than the window, so an image of what is in view would be cut off.
    const natural = await page.locator('.orgchart').evaluate((element) => ({ width: (element as HTMLElement).offsetWidth, height: (element as HTMLElement).offsetHeight }))
    expect(natural.width).toBeGreaterThan(1200)
    const download = page.waitForEvent('download')
    await page.getByRole('button', { name: 'PNG' }).click()
    const file = await download
    expect(file.suggestedFilename()).toBe('org-chart-all-orgs.png')
    const bytes = readFileSync((await file.path()) ?? '')
    const { width, height } = pngSize(bytes)
    // The whole chart and its padding, at one pixel ratio on both sides.
    expect(width / height).toBeCloseTo((natural.width + 48) / (natural.height + 48), 1)
    expect(width).toBeGreaterThanOrEqual(natural.width + 48)
    const seen = await inspectPng(page, bytes)
    const brightness = (seen.corner[0]! + seen.corner[1]! + seen.corner[2]!) / 3
    if (theme === 'dark') expect(brightness).toBeLessThan(60)
    else expect(brightness).toBeGreaterThan(200)
    // The boxes, lines, and text are in it, not a blank page.
    expect(seen.different).toBeGreaterThan(width * height * 0.01)
  })
}

test('downloads the whole chart as an SVG', async ({ page }) => {
  await page.goto('/org?org=acme')
  await expect(page.locator('.orgchart .oc-node.agent').first()).toBeVisible()
  const download = page.waitForEvent('download')
  await page.getByRole('button', { name: 'SVG' }).click()
  const file = await download
  expect(file.suggestedFilename()).toBe('org-chart-acme.svg')
  const text = readFileSync((await file.path()) ?? '', 'utf8')
  expect(text).toContain('<foreignObject')
  expect(text).toContain('Engineering')
})

test('fits the chart to the width and zooms it', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 })
  await page.goto('/org')
  const percent = page.getByRole('button', { name: 'Actual size' })
  await expect(percent).toBeVisible()
  await page.getByRole('button', { name: 'Fit to width' }).click()
  const fitted = Number.parseInt((await percent.textContent()) ?? '0', 10)
  expect(fitted).toBeLessThan(100)
  const frame = page.locator('.oc-frame')
  // Fitted to the width, nothing of the chart is out of the frame sideways.
  expect(await frame.evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true)
  await page.getByRole('button', { name: 'Zoom in' }).click()
  expect(Number.parseInt((await percent.textContent()) ?? '0', 10)).toBeGreaterThan(fitted)
  await percent.click()
  await expect(percent).toHaveText('100%')
})
