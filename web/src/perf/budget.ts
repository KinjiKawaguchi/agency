// What the built client weighs, and the budget it is held to (web/budget.json). The sizes are what the browser downloads:
// gzip, with the bytes before it for reference. The numbers of the budget are today's with a margin, so a change that makes the
// client heavier has to say so, in the budget, in the same pull request.

export interface Asset {
  /** The file name under dist/assets, with its hash. */
  file: string
  kind: 'js' | 'css'
  /** Loaded with the page itself (`index.html` names it), not when a screen asks for it. */
  isInitial: boolean
  raw: number
  gzip: number
}

export interface Budget {
  /** What every page loads before anything is drawn, in gzip bytes. */
  initialJsGzip: number
  initialCssGzip: number
  /** A chunk loaded when a screen asks for it, by its name without the hash; `chunkDefaultGzip` for one that is not named. */
  chunkGzip: Record<string, number>
  chunkDefaultGzip: number
  /** All the script of the client, initial and lazy, in gzip bytes. */
  totalJsGzip: number
  /** Milliseconds from the navigation to the first request drawn on the desk, in the fixed setup of e2e/load.spec.ts (median). */
  firstDeskMs: { cold: number; warm: number }
}

/** `Markdown-BvnFk_b2.js` is `Markdown`: the hash Vite adds is dropped. */
export const chunkName = (file: string): string => file.replace(/\.(js|css)$/, '').replace(/-[A-Za-z0-9_-]{8}$/, '')

const kb = (bytes: number): string => `${(bytes / 1000).toFixed(1)} kB`

const sum = (assets: readonly Asset[], pick: (asset: Asset) => number): number => assets.reduce((total, one) => total + pick(one), 0)

/** What is over the budget, one sentence for each; nothing when the client is within it. */
export function overBudget(assets: readonly Asset[], budget: Budget): string[] {
  const found: string[] = []
  const initialJs = sum(assets.filter((one) => one.kind === 'js' && one.isInitial), (one) => one.gzip)
  const initialCss = sum(assets.filter((one) => one.kind === 'css' && one.isInitial), (one) => one.gzip)
  const totalJs = sum(assets.filter((one) => one.kind === 'js'), (one) => one.gzip)
  if (initialJs > budget.initialJsGzip) found.push(`The script every page loads is ${kb(initialJs)} gzipped; the budget is ${kb(budget.initialJsGzip)}.`)
  if (initialCss > budget.initialCssGzip) found.push(`The style every page loads is ${kb(initialCss)} gzipped; the budget is ${kb(budget.initialCssGzip)}.`)
  if (totalJs > budget.totalJsGzip) found.push(`All the script is ${kb(totalJs)} gzipped; the budget is ${kb(budget.totalJsGzip)}.`)
  for (const one of assets.filter((asset) => !asset.isInitial && asset.kind === 'js')) {
    const name = chunkName(one.file)
    const limit = Object.hasOwn(budget.chunkGzip, name) ? budget.chunkGzip[name] : undefined
    const allowed = limit ?? budget.chunkDefaultGzip
    if (one.gzip > allowed) found.push(`The chunk ${name} is ${kb(one.gzip)} gzipped; ${limit === undefined ? 'a chunk with no name in the budget may be' : 'the budget is'} ${kb(allowed)}.`)
  }
  return found
}

/** The budget with a margin over what was measured: a tenth more, rounded up to a kilobyte, so an ordinary change does not trip it. */
export const withMargin = (measured: number): number => Math.ceil((measured * 1.1) / 1000) * 1000
