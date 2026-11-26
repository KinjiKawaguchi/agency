import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

// A character that reorders or hides how a line reads (the set the screens print as `?`, src/shared/printable.ts) must never be in
// the source as itself, where a reader cannot see it and an editor may draw the code in another order: it is written `\uXXXX`.
const HIDDEN = /[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F-\u009F\u061C\u200B\u200E\u200F\u202A-\u202E\u2060\u2066-\u2069\uFEFF]/
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
      return text.split('\n').flatMap((line, index) => (HIDDEN.test(line) ? [`${file.replace(process.cwd() + '/', '')}:${index + 1}`] : []))
    })
    expect(found).toEqual([])
  })
})
