import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { pageScrollsSideways, resetNode } from './helpers'

// The Japanese names of the menu say what each screen is for (docs/glossary.md, "Menu and screen names"); the menu has to hold them at every width.
const DIRECTORY = 'e2e-screenshots'

for (const [theme, width, height] of [['light', 1280, 800], ['dark', 1280, 800], ['light', 390, 844], ['dark', 390, 844]] as const) {
  test(`names the menu and its groups in Japanese, in ${theme} at ${width}px, with nothing cut off or scrolling sideways`, async ({ page, request }) => {
    mkdirSync(DIRECTORY, { recursive: true })
    await resetNode(request, { evaluation: true, outcomes: true, members: true, history: 3 })
    await page.setViewportSize({ width, height })
    await page.goto(`/org?lang=ja&theme=${theme}`)
    const main = page.getByRole('navigation', { name: 'メインメニュー' })
    await expect(main).toBeVisible()
    for (const name of ['あなた宛てのリクエスト', 'チームの状況', '組織', '成果']) await expect(main.getByRole('link', { name })).toBeVisible()
    const group = page.getByRole('navigation', { name: '組織のページ' })
    for (const name of ['組織図', 'メンバーの作業状況', '組織のメンバー', '組織の変更履歴']) await expect(group.getByRole('link', { name })).toBeVisible()
    expect(await pageScrollsSideways(page)).toBe(false)
    await page.screenshot({ path: `${DIRECTORY}/menu-ja-organization-${theme}-${width}.png`, clip: { x: 0, y: 0, width, height: 240 }, animations: 'disabled' })
    await page.goto(`/costs?lang=ja&theme=${theme}`)
    const results = page.getByRole('navigation', { name: '成果のページ' })
    for (const name of ['コスト', '評価', 'ライブ', 'KPI']) await expect(results.getByRole('link', { name })).toBeVisible()
    expect(await pageScrollsSideways(page)).toBe(false)
    await page.screenshot({ path: `${DIRECTORY}/menu-ja-results-${theme}-${width}.png`, clip: { x: 0, y: 0, width, height: 240 }, animations: 'disabled' })
  })
}
