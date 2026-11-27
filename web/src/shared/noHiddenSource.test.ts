import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { hiddenRanges, sharedHidden } from '../test/hiddenChars'

// A character that reorders or hides how a line reads (the set the screens print as `?`: testdata/label-hidden-chars.json) must never be in
// the source as itself, where a reader cannot see it and an editor may draw the code in another order: it is written `\uXXXX`. A tab and
// the line breaks are the shared file's `textKeeps`: source has them.
const KEPT = new Set(sharedHidden.textKeeps.map((one) => Number.parseInt(one, 16)))
const isHidden = (character: string): boolean => {
  const code = character.codePointAt(0) ?? 0
  return !KEPT.has(code) && hiddenRanges.some(([from, to]) => code >= from && code <= to)
}
const ROOTS = ['src', 'e2e', 'public', 'scripts', 'vite']

function files(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) return entry.name === 'node_modules' ? [] : files(path)
    return /\.(ts|tsx|js|mjs|css|html|json)$/.test(entry.name) ? [path] : []
  })
}

describe('the source of the client', () => {
  it('has no character that hides or reorders a line, as itself', () => {
    const found = ROOTS.flatMap((root) => files(join(process.cwd(), root))).flatMap((file) => {
      const text = readFileSync(file, 'utf8')
      return text.split('\n').flatMap((line, index) => ([...line].some(isHidden) ? [`${file.replace(process.cwd() + '/', '')}:${index + 1}`] : []))
    })
    expect(found).toEqual([])
  })
})
