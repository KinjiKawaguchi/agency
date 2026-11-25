import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { pageScrollsSideways, resetNode } from './helpers'

// A screenshot of every screen, in both themes, at desktop and phone width. CI keeps them as an artifact
// so a reviewer can see what a change did to the screens without running the client.
const DIRECTORY = 'e2e-screenshots'

const VIEWPORTS = [
  { name: 'desktop', width: 1440, height: 900 },
  { name: 'phone', width: 390, height: 844 },
] as const

const THEMES = ['light', 'dark'] as const

const SCREENS = [
  { name: 'desk', path: '/desk', ready: 'article.paper' },
  { name: 'overview', path: '/overview', ready: 'article.ov-team' },
  { name: 'people', path: '/people', ready: 'article.mate' },
  { name: 'org', path: '/org', ready: '.orgchart .oc-node.agent' },
  { name: 'costs', path: '/costs', ready: '.columns .column' },
  { name: 'devices', path: '/devices', ready: '.pairing-code' },
] as const

test.beforeAll(() => mkdirSync(DIRECTORY, { recursive: true }))

for (const viewport of VIEWPORTS) {
  for (const theme of THEMES) {
    for (const screen of SCREENS) {
      test(`${screen.name} in ${theme} at ${viewport.name} width`, async ({ page, request }) => {
        // The overview shows several orgs, with a stuck team, a stale copy, and one that cannot be read.
        await resetNode(request, { usage: true, moreOrgs: screen.name === 'overview' })
        await page.setViewportSize({ width: viewport.width, height: viewport.height })
        await page.goto(`${screen.path}?theme=${theme}`)
        await expect(page.locator(screen.ready).first()).toBeVisible()
        // The layout holds: nothing makes the page itself scroll sideways.
        expect(await pageScrollsSideways(page)).toBe(false)
        await page.screenshot({ path: `${DIRECTORY}/${screen.name}-${theme}-${viewport.name}.png`, fullPage: true, animations: 'disabled' })
      })
    }
  }
}
