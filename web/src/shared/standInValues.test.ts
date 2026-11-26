import { readdirSync, readFileSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'

// The rule (CLAUDE.md): a screen never shows a placeholder for something the node has not answered yet. A default that
// stands in for a value (`?? 0`, `|| []`, state that starts as a number or a list) is how a screen shows "0" or "nothing"
// before the answer, so nobody can tell loading from really 0. This reads the source of the app and finds every one;
// each must be named with its kind and the reason it is not a stand-in for a node's value, and a name with no
// default left to name fails too, so the list stays true.
//
//   // stand-in-ok(<kind>): <reason>      on the line of the default or the line before it
const KINDS = {
  scale: 'the geometry or scale of a chart that exists only after the answer (an axis maximum, a width): no node value is shown by it',
  sparse: 'a day or a type the node leaves out of its series because it has nothing: 0 is what the node says, and it is drawn only in what exists after the answer',
  optional: 'a field the node leaves out when it is empty, read from an answer that has come',
  routing: 'where a tap goes or what is drawn around a value, never a value that is shown',
  index: 'an index, a character code, or an accumulator of the code itself: no value of the node',
  device: 'the state of this device or of a form the viewer fills in, not the node’s',
} as const

const SKIPPED_DIRS = new Set(['mock', 'test'])
const isSource = (name: string): boolean => /\.(ts|tsx)$/.test(name) && !/\.test\.(ts|tsx)$/.test(name) && !name.endsWith('.d.ts')

function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    if (entry.isDirectory()) return SKIPPED_DIRS.has(entry.name) ? [] : sources(join(dir, entry.name))
    return isSource(entry.name) ? [join(dir, entry.name)] : []
  })
}

/** A default that could stand in for a value. */
const DEFAULTS: readonly RegExp[] = [
  /(\?\?|\|\|)\s*(0|\[\]|''|""|\{\}|false|true)(?![\w.])/,
  /\buseState(<[^>]*>)?\(\s*(0|\[\]|''|""|\{\}|new Map\(\)|new Set\(\))\s*\)/,
  /\b\w+\s*=\s*(0|\[\]|''|"")\s*[,})]/,
]

const MARKER = /stand-in-ok\((\w+)\):\s*(.*\S)/

interface Hit {
  file: string
  line: number
  text: string
}

function found(): { hits: Hit[]; markers: Array<{ file: string; line: number; kind: string; reason: string }> } {
  const root = join(process.cwd(), 'src')
  const hits: Hit[] = []
  const markers: Array<{ file: string; line: number; kind: string; reason: string }> = []
  for (const path of sources(root)) {
    const file = relative(process.cwd(), path)
    readFileSync(path, 'utf8')
      .split('\n')
      .forEach((text, index) => {
        const marker = MARKER.exec(text)
        if (marker !== null) markers.push({ file, line: index + 1, kind: marker[1] ?? '', reason: marker[2] ?? '' })
        const code = text.replace(/\/\/.*$/, '')
        if (DEFAULTS.some((pattern) => pattern.test(code))) hits.push({ file, line: index + 1, text: text.trim() })
      })
  }
  return { hits, markers }
}

describe('placeholder values', () => {
  const { hits, markers } = found()
  const marked = (hit: Hit): boolean => markers.some((marker) => marker.file === hit.file && (marker.line === hit.line || marker.line === hit.line - 1))

  it('are never a default for something the node has said: every default of the app is named with its kind', () => {
    expect(hits.filter((hit) => !marked(hit)).map((hit) => `${hit.file}:${hit.line}  ${hit.text}`)).toEqual([])
  })

  it('are named only where there is one, with a kind that is known and a reason', () => {
    const used = (marker: (typeof markers)[number]): boolean => hits.some((hit) => hit.file === marker.file && (marker.line === hit.line || marker.line === hit.line - 1))
    expect(markers.filter((marker) => !used(marker)).map((marker) => `${marker.file}:${marker.line} names a default that is gone`)).toEqual([])
    expect(markers.filter((marker) => !(marker.kind in KINDS)).map((marker) => `${marker.file}:${marker.line} kind ${marker.kind}`)).toEqual([])
  })

  it('has a reason for each kind, so a name says what it is not', () => {
    for (const reason of Object.values(KINDS)) expect(reason.length).toBeGreaterThan(20)
  })
})
