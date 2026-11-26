// Draws the app's PNG icons from the seal marks in public/icons/*.svg, with the headless Chromium Playwright
// already uses for the end-to-end tests: `AGENCY_CHROMIUM_PATH=<path> node scripts/make-icons.mjs`. The PNGs are
// committed; run this only when a mark changes.
import { readFileSync, writeFileSync } from 'node:fs'
import { chromium } from '@playwright/test'

const OUT = [
  { from: 'seal.svg', to: 'icon-192.png', size: 192 },
  { from: 'seal.svg', to: 'icon-512.png', size: 512 },
  { from: 'seal.svg', to: 'apple-touch-icon.png', size: 180 },
  { from: 'seal-maskable.svg', to: 'icon-maskable-512.png', size: 512 },
]

const executablePath = process.env.AGENCY_CHROMIUM_PATH
const browser = await chromium.launch(executablePath === undefined ? {} : { executablePath })
for (const { from, to, size } of OUT) {
  const page = await browser.newPage({ viewport: { width: size, height: size } })
  const svg = readFileSync(new URL(`../public/icons/${from}`, import.meta.url), 'utf8')
  await page.setContent(`<body style="margin:0;background:transparent">${svg.replace('width="512" height="512"', `width="${size}" height="${size}"`)}</body>`)
  writeFileSync(new URL(`../public/icons/${to}`, import.meta.url), await page.screenshot({ omitBackground: true, type: 'png' }))
  await page.close()
}
await browser.close()
