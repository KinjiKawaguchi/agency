import { chunkName, overBudget, withMargin, type Asset, type Budget } from './budget'

const asset = (file: string, gzip: number, isInitial = false, kind: Asset['kind'] = 'js'): Asset => ({ file, kind, isInitial, raw: gzip * 3, gzip })
const budget: Budget = { initialJsGzip: 100_000, initialCssGzip: 10_000, chunkGzip: { Markdown: 50_000 }, chunkDefaultGzip: 20_000, totalJsGzip: 160_000, firstDeskMs: { cold: 3000, warm: 1500 } }
const within = [asset('index-AbCdEfGh.js', 90_000, true), asset('index-AbCdEfGh.css', 9_000, true, 'css'), asset('Markdown-12345678.js', 48_000)]

describe('the weight of the client', () => {
  it('is within the budget when nothing is over it, and a limit may be met exactly', () => {
    expect(overBudget(within, budget)).toEqual([])
    expect(overBudget([asset('index-AbCdEfGh.js', 100_000, true), asset('index-AbCdEfGh.css', 10_000, true, 'css')], budget)).toEqual([])
  })

  it('names the script every page loads when it is one byte over', () => {
    const found = overBudget([asset('index-AbCdEfGh.js', 100_001, true)], budget)
    expect(found).toHaveLength(1)
    expect(found[0]).toContain('script every page loads')
  })

  it('names the style every page loads when it is over', () => {
    expect(overBudget([asset('index-AbCdEfGh.css', 10_001, true, 'css')], budget)[0]).toContain('style every page loads')
  })

  it('holds a lazy chunk to the limit it has by name, and one with no name to the default, and says which', () => {
    expect(overBudget([asset('Markdown-12345678.js', 50_001)], budget)[0]).toMatch(/chunk Markdown is 50\.0 kB gzipped; the budget is 50\.0 kB/)
    expect(overBudget([asset('Markdown-12345678.js', 50_000)], budget)).toEqual([])
    expect(overBudget([asset('Evaluation-12345678.js', 20_001)], budget)[0]).toMatch(/chunk Evaluation .* a chunk with no name in the budget may be 20\.0 kB/)
    expect(overBudget([asset('Evaluation-12345678.js', 20_000)], budget)).toEqual([])
  })

  it('does not take the name of a chunk for a property of an object', () => {
    expect(overBudget([asset('constructor-12345678.js', 20_001)], budget)[0]).toContain('a chunk with no name in the budget')
  })

  it('holds all the script together to its own limit, whatever the parts are', () => {
    const parts = [asset('index-AbCdEfGh.js', 99_000, true), asset('Markdown-12345678.js', 49_000), asset('A-12345678.js', 12_001)]
    expect(overBudget(parts, budget)).toEqual([expect.stringContaining('All the script is 160.0 kB')])
  })

  it('counts a style among none of the script', () => {
    expect(overBudget([asset('index-AbCdEfGh.js', 100_000, true), asset('x-12345678.css', 200_000, false, 'css')], budget)).toEqual([])
  })
})

describe('the name of a chunk', () => {
  it('is the file without its hash and its extension', () => {
    expect(['Markdown-BvnFk_b2.js', 'index-CkEdG-6w.css', 'EvaluationPage-A_-1b2c3.js'].map(chunkName)).toEqual(['Markdown', 'index', 'EvaluationPage'])
    expect(chunkName('plain.js')).toBe('plain')
  })
})

describe('the margin of a budget', () => {
  it('is a tenth over what was measured, rounded up to the kilobyte', () => {
    expect([184_050, 10_101, 48_648, 232_698].map(withMargin)).toEqual([203_000, 12_000, 54_000, 256_000])
    expect(withMargin(1000)).toBe(2000)
  })
})
