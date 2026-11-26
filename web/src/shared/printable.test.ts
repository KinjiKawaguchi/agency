import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { labelsMadePrintable, printable } from './printable'

// The set and the cases are in one file that the other clients and the node read too (testdata/label-hidden-chars.json), so that
// no one of them can drift from the others.
const shared = JSON.parse(readFileSync(join(process.cwd(), '..', 'testdata', 'label-hidden-chars.json'), 'utf8')) as {
  mark: string
  hidden: Array<[string, string]>
  cases: Array<{ text: string; printable: string }>
}
const ranges = shared.hidden.map(([from, to]) => [Number.parseInt(from, 16), Number.parseInt(to, 16)] as const)
const isHidden = (code: number): boolean => ranges.some(([from, to]) => code >= from && code <= to)

describe('text that someone else wrote, made printable', () => {
  it('prints every character of the set as the mark, edges of the ranges included', () => {
    for (const [from, to] of ranges) for (let code = from; code <= to; code++) expect(printable(String.fromCodePoint(code)), `U+${code.toString(16)}`).toBe(shared.mark)
  })

  it('leaves every other character of the basic plane alone, the joiners included', () => {
    const changed: string[] = []
    for (let code = 0; code <= 0xffff; code++) if (!isHidden(code) && printable(String.fromCharCode(code)) !== String.fromCharCode(code)) changed.push(code.toString(16))
    expect(changed).toEqual([])
  })

  it.each(shared.cases.map((one, index) => [index, one] as const))('says case %i of the shared file as the file does', (_index, one) => {
    expect(printable(one.text)).toBe(one.printable)
  })

  it('has the cases the file is for: a hidden character and the joiners that stay', () => {
    expect(shared.cases.some((one) => one.printable !== one.text)).toBe(true)
    expect(shared.cases.some((one) => one.text.includes(String.fromCodePoint(0x200d)) && one.printable === one.text)).toBe(true)
    expect(shared.cases.some((one) => one.text.includes(String.fromCodePoint(0xe0067)) && one.printable === one.text)).toBe(true)
  })

  it('puts a mark for each, in a line with several, and turns a reordering override into marks', () => {
    const rlo = String.fromCodePoint(0x202e)
    expect(printable(`a${rlo}b${String.fromCodePoint(0x202a)}c`)).toBe('a?b?c')
    expect(printable(`Alice${rlo}ecil`)).toBe('Alice?ecil')
  })

  it('has nothing to say of an empty text', () => {
    expect(printable('')).toBe('')
  })
})

describe('the labels of an answer, made printable wherever they lie', () => {
  const dirty = `Al${String.fromCodePoint(0x202e)}ce`
  it('turns a label, a label of a choice, and each label of a list and of what lies in it', () => {
    const answer = { account: { id: 'alice', label: dirty }, orgs: [{ id: 'acme', label: dirty, teams: [{ id: 'app', label: dirty }] }], entries: [{ choiceLabel: dirty, relayedFor: { id: 'finance', label: dirty } }] }
    expect(labelsMadePrintable(answer)).toEqual({ account: { id: 'alice', label: 'Al?ce' }, orgs: [{ id: 'acme', label: 'Al?ce', teams: [{ id: 'app', label: 'Al?ce' }] }], entries: [{ choiceLabel: 'Al?ce', relayedFor: { id: 'finance', label: 'Al?ce' } }] })
  })

  it('turns the labels of a snapshot’s map of whom a request is for, which are its values', () => {
    expect(labelsMadePrintable({ relayedFor: { 'r-1': dirty, 'r-2': 'Erin' } })).toEqual({ relayedFor: { 'r-1': 'Al?ce', 'r-2': 'Erin' } })
  })

  it('leaves what is not a label alone: a title, a text, an id, and a label that is not text', () => {
    const answer = { title: dirty, text: dirty, id: dirty, label: 7, other: { label: null }, relayedFor: ['a', dirty] }
    expect(labelsMadePrintable(answer)).toEqual({ ...answer, relayedFor: ['a', dirty] })
  })

  it('does not change the answer it was given, and keeps what is not an object', () => {
    const answer = { label: dirty }
    labelsMadePrintable(answer)
    expect(answer.label).toBe(dirty)
    expect([null, 3, 'x', true].map(labelsMadePrintable)).toEqual([null, 3, 'x', true])
  })
})
