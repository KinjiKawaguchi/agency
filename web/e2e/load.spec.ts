import { appendFileSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test, type Browser, type Page } from '@playwright/test'
import { resetNode } from './helpers'

// How long the built client takes to draw the first request on the desk against the mock node, in one fixed setup so that a number
// means the same on every run: headless Chromium with the processor slowed 4 times (a phone is not a laptop), the node on this
// machine (the network is not what is measured), the median of 5 loads. Cold is a browser that has never seen the page; warm is the
// same browser loading it again, with whatever it kept. The budget is in budget.json, with the sizes of the bundle.
const RUNS = 5
const SLOWDOWN = 4

interface Load {
  /** Milliseconds from the navigation to the first paint of any content. */
  firstContentfulPaintMs: number
  /** Milliseconds from the navigation to the first request of the pile in the document. */
  firstDeskMs: number
  /** Whether the page was served through the worker, as a person who comes back is. */
  isControlled: boolean
}

declare global {
  interface Window {
    __firstDeskMs?: number
  }
}

async function slowDown(page: Page): Promise<void> {
  const session = await page.context().newCDPSession(page)
  await session.send('Emulation.setCPUThrottlingRate', { rate: SLOWDOWN })
}

async function load(page: Page): Promise<Load> {
  await page.goto('/desk', { waitUntil: 'commit' })
  await page.waitForFunction(() => window.__firstDeskMs !== undefined, undefined, { timeout: 60_000 })
  return page.evaluate(() => ({
    firstContentfulPaintMs: performance.getEntriesByName('first-contentful-paint')[0]?.startTime ?? -1,
    firstDeskMs: window.__firstDeskMs ?? -1,
    isControlled: navigator.serviceWorker.controller !== null,
  }))
}

/** Notes the moment the first request of the pile is in the document, as the page's own clock says it (not the poll of the test). */
async function watchTheDesk(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const seen = new MutationObserver(() => {
      if (document.querySelector('[role="option"]') !== null) {
        window.__firstDeskMs = performance.now()
        seen.disconnect()
      }
    })
    seen.observe(document, { childList: true, subtree: true })
  })
}

const median = (values: readonly number[]): number => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ?? -1
const spread = (values: readonly number[]): string => `${Math.min(...values).toFixed(0)}/${median(values).toFixed(0)}/${Math.max(...values).toFixed(0)}`

async function cold(browser: Browser): Promise<Load[]> {
  const loads: Load[] = []
  for (let run = 0; run < RUNS; run++) {
    const context = await browser.newContext({ baseURL: test.info().project.use.baseURL })
    const page = await context.newPage()
    await watchTheDesk(page)
    await slowDown(page)
    loads.push(await load(page))
    await context.close()
  }
  return loads
}

async function warm(browser: Browser): Promise<Load[]> {
  const context = await browser.newContext({ baseURL: test.info().project.use.baseURL })
  const page = await context.newPage()
  await watchTheDesk(page)
  await slowDown(page)
  await load(page)
  // Warm is the load of a person who comes back: the worker is installed and serves the page. Not a wait of a guessed length: the
  // worker says when it is ready, and a load after that is the first one it controls.
  const ready = await page.evaluate(() => Promise.race([navigator.serviceWorker.ready.then(() => true), new Promise<boolean>((resolve) => setTimeout(() => resolve(false), 15_000))]))
  expect(ready, 'the worker is installed').toBe(true)
  await load(page)
  const loads: Load[] = []
  for (let run = 0; run < RUNS; run++) loads.push(await load(page))
  await context.close()
  expect(loads.every((one) => one.isControlled), 'every warm load is served through the worker').toBe(true)
  return loads
}

test('draws the first request of the desk within the budget, cold and warm', async ({ browser, request }) => {
  test.setTimeout(300_000)
  await resetNode(request)
  const coldLoads = await cold(browser)
  const warmLoads = await warm(browser)
  const numbers = {
    cold: { firstContentfulPaintMs: median(coldLoads.map((one) => one.firstContentfulPaintMs)), firstDeskMs: median(coldLoads.map((one) => one.firstDeskMs)) },
    warm: { firstContentfulPaintMs: median(warmLoads.map((one) => one.firstContentfulPaintMs)), firstDeskMs: median(warmLoads.map((one) => one.firstDeskMs)) },
  }
  const line = `load (processor ${SLOWDOWN}x slower, median of ${RUNS}): cold first paint ${numbers.cold.firstContentfulPaintMs.toFixed(0)} ms, first desk ${numbers.cold.firstDeskMs.toFixed(0)} ms; warm first paint ${numbers.warm.firstContentfulPaintMs.toFixed(0)} ms, first desk ${numbers.warm.firstDeskMs.toFixed(0)} ms (first desk, min/median/max of ${RUNS}: cold ${spread(coldLoads.map((one) => one.firstDeskMs))}; warm ${spread(warmLoads.map((one) => one.firstDeskMs))}; first paint: cold ${spread(coldLoads.map((one) => one.firstContentfulPaintMs))}; warm ${spread(warmLoads.map((one) => one.firstContentfulPaintMs))})`
  console.log(line)
  if (process.env.GITHUB_STEP_SUMMARY !== undefined) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `\n**Web client load**: ${line}\n`)
  const budget = JSON.parse(readFileSync(join(import.meta.dirname, '..', 'budget.json'), 'utf8')) as { firstDeskMs: { cold: number; warm: number } }
  expect(numbers.cold.firstDeskMs, `cold: the first request of the desk. ${line}`).toBeLessThan(budget.firstDeskMs.cold)
  expect(numbers.warm.firstDeskMs, `warm: the first request of the desk. ${line}`).toBeLessThan(budget.firstDeskMs.warm)
})
