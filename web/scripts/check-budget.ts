// Weighs the built client (dist/) and fails when it is over the budget (budget.json). Run by `npm run build`; `--table` prints the
// numbers and never fails, for a reader who wants them.
import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { brotliCompressSync, constants, gzipSync } from 'node:zlib'
import { chunkName, overBudget, type Asset, type Budget } from '../src/perf/budget.ts'

const dist = join(import.meta.dirname, '..', 'dist')
const index = readFileSync(join(dist, 'index.html'), 'utf8')
const initial = new Set([...index.matchAll(/(?:src|href)="\/assets\/([^"]+)"/g)].map((found) => found[1] ?? ''))

const assets: Asset[] = readdirSync(join(dist, 'assets'))
  .filter((file) => /\.(js|css)$/.test(file))
  .map((file) => {
    const bytes = readFileSync(join(dist, 'assets', file))
    return { file, kind: file.endsWith('.css') ? 'css' : 'js', isInitial: initial.has(file), raw: bytes.length, gzip: gzipSync(bytes, { level: 9 }).length } satisfies Asset
  })
  .sort((a, b) => Number(b.isInitial) - Number(a.isInitial) || b.gzip - a.gzip)

const kb = (bytes: number): string => (bytes / 1000).toFixed(1).padStart(7)
console.log('\nWhat the browser downloads (kB):    raw   gzip  brotli')
for (const one of assets) {
  const brotli = brotliCompressSync(readFileSync(join(dist, 'assets', one.file)), { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }).length
  console.log(`${one.isInitial ? 'every page ' : 'on demand  '}${chunkName(one.file).padEnd(22)} ${kb(one.raw)} ${kb(one.gzip)} ${kb(brotli)}`)
}
if (process.argv.includes('--table')) process.exit(0)
const budget = JSON.parse(readFileSync(join(import.meta.dirname, '..', 'budget.json'), 'utf8')) as Budget
const found = overBudget(assets, budget)
if (found.length > 0) {
  console.error(`\nThe client is over its budget (web/budget.json):\n- ${found.join('\n- ')}\nIf the weight is meant, say so in budget.json in the same pull request.`)
  process.exit(1)
}
console.log('Within the budget (web/budget.json).')
