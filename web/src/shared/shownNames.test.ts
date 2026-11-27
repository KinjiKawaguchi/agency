import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

// A label of the node is made printable once, where an answer is read (src/api/client.ts). A name of the team's runtime (who drafted a
// request, who it is for, who a message is from) is drawn as a person's name too, and it is not a label of the org: it is made
// printable where it is drawn, with `printable`. This finds every place the code reaches such a name, and each must either draw it
// through `printable(` on that line, or say why it is not drawn as it is:
//
//   /* shown-ok(<kind>): <reason> */      on the line; the kind says what the `from`/`to` there is, since it is not a runtime's name:
const KIND_MEANINGS = {
  lookup: 'the name is only used to find a person; what is drawn is the person label or `printable(name)`',
  key: 'the name is only a React key or an id, never drawn',
  status: 'the `from`/`to` of an event that moves a work item between states: a state word of the node, drawn through the table of states, never a name',
  passed: 'the value is handed on to a component that draws it through `printable(`',
  type: 'a field declared in a type (the value is drawn through `printable(` where the component uses it)',
  route: 'a router location (`to` of a link or a route target): a path of the app, never a name',
  period: 'a `from` of a range of days or times (the first day), never a name',
} as const
const KINDS = Object.keys(KIND_MEANINGS) as ReadonlyArray<keyof typeof KIND_MEANINGS>
// Any `.from` or `.to` that is not a call, on any object (an alias of a request or an event cannot slip past by its name), a `from` or
// a `to` taken out of an object or a list, and the other names of a runtime.
const RUNTIME_NAME = /\.(from|to)\b(?!\()|[{[][^}\]]*\b(from|to)\b[^}\]]*[}\]]\s*=[^=>]|\b(participant\.name|draftedBy)\b/
const SKIPPED = new Set(['mock', 'test'])

function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    if (entry.isDirectory()) return SKIPPED.has(entry.name) ? [] : sources(join(dir, entry.name))
    return /\.(ts|tsx)$/.test(entry.name) && !/\.test\.(ts|tsx)$/.test(entry.name) && !entry.name.endsWith('.d.ts') ? [join(dir, entry.name)] : []
  })
}

const root = join(process.cwd(), 'src')
const lines = sources(root).flatMap((file) => readFileSync(file, 'utf8').split('\n').map((text, index) => ({ at: `${file.replace(`${process.cwd()}/`, '')}:${index + 1}`, text })))

describe('what the guard finds', () => {
  it.each([
    'const who = alias.from',
    'draw(item.to)',
    'x?.from ?? y',
    'const { from, to } = message',
    'const { id, from } = event',
    'const [from, to] = [a, b]',
    'const [ref, title, from, to] = parts',
    'ready.participant.name',
    'draftedBy={x}',
  ])('finds %s', (line) => {
    expect(RUNTIME_NAME.test(line)).toBe(true)
  })

  it.each(['Array.from(items)', 'Buffer.from(text)', 'router.to(path)', 'const fromDate = 1', 'const toast = 2', 'const { fromage } = x', 'const [a, b] = list', 'x.fromAt', 'a.tooBig'])('does not take %s for a name', (line) => {
    expect(RUNTIME_NAME.test(line)).toBe(false)
  })
})

describe('the names of the runtime that are drawn as people', () => {
  it('are drawn through `printable`, or named with why they are not drawn as they are', () => {
    const hits = lines.filter((line) => RUNTIME_NAME.test(line.text) && !/^\s*(\/\/|\*|\/\*)/.test(line.text))
    const unnamed = hits.filter((line) => !line.text.includes('printable(') && !new RegExp(`shown-ok\\((${KINDS.join('|')})\\): \\S`).test(line.text))
    expect(unnamed.map((line) => `${line.at}  ${line.text.trim()}`)).toEqual([])
  })

  it('are named only where there is such a name, with a kind that is known and a reason', () => {
    const named = lines.filter((line) => line.text.includes('shown-ok('))
    const bad = named.filter((line) => !RUNTIME_NAME.test(line.text) || !new RegExp(`shown-ok\\((${KINDS.join('|')})\\): \\S`).test(line.text))
    expect(bad.map((line) => `${line.at}  ${line.text.trim()}`)).toEqual([])
  })
})

describe('the answers of the node', () => {
  it('are read in one place, the client, which makes every label printable: nothing else fetches or reads a response', () => {
    const readers = lines.filter((line) => /\bfetch\(|\.json\(\)/.test(line.text) && !/^\s*(\/\/|\*|\/\*)/.test(line.text) && !line.at.startsWith('src/api/client.ts') && !/\bquery\.fetch\(\)/.test(line.text))
    expect(readers.map((line) => `${line.at}  ${line.text.trim()}`)).toEqual([])
  })
})
