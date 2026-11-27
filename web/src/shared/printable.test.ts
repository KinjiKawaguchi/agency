import { hiddenRanges, sharedHidden } from '../test/hiddenChars'
import { asGiven, labelsMadePrintable, printable, printableText } from './printable'

/** What is drawn: the answer without the `asGiven` it keeps of what the node wrote (tested on its own below). */
const shownOf = (value: unknown): unknown => JSON.parse(JSON.stringify(value, (key, inside) => (key === 'asGiven' ? undefined : inside)))

// The set and the cases are in one file that the other clients and the node read too (testdata/label-hidden-chars.json), so that
// no one of them can drift from the others.
const shared = sharedHidden
const ranges = hiddenRanges
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
    expect(shownOf(labelsMadePrintable(answer))).toEqual({ account: { id: 'alice', label: 'Al?ce' }, orgs: [{ id: 'acme', label: 'Al?ce', teams: [{ id: 'app', label: 'Al?ce' }] }], entries: [{ choiceLabel: 'Al?ce', relayedFor: { id: 'finance', label: 'Al?ce' } }] })
  })

  it('turns the labels of a snapshot’s map of whom a request is for, which are its values', () => {
    expect(labelsMadePrintable({ relayedFor: { 'r-1': dirty, 'r-2': 'Erin' } })).toEqual({ relayedFor: { 'r-1': 'Al?ce', 'r-2': 'Erin' } })
  })

  it('leaves what is not a label alone: an id, a name, a label that is not text, and a map that is not one', () => {
    const answer = { id: dirty, from: dirty, name: dirty, label: 7, other: { label: null }, relayedFor: ['a', dirty] }
    expect(labelsMadePrintable(answer)).toEqual({ ...answer, relayedFor: ['a', dirty] })
  })

  it('does not change the answer it was given, and keeps what is not an object', () => {
    const answer = { label: dirty }
    labelsMadePrintable(answer)
    expect(answer.label).toBe(dirty)
    expect([null, 3, 'x', true].map(labelsMadePrintable)).toEqual([null, 3, 'x', true])
  })
})

describe('a text that an agent or a person wrote, made printable', () => {
  const keeps = sharedHidden.textKeeps.map((one) => Number.parseInt(one, 16))

  it('prints every character of the set as the mark, except the tab and the line breaks, which stay', () => {
    for (const [from, to] of ranges) {
      for (let code = from; code <= to; code++) expect(printableText(String.fromCodePoint(code)), `U+${code.toString(16)}`).toBe(keeps.includes(code) ? String.fromCodePoint(code) : sharedHidden.mark)
    }
  })

  it('leaves every other character of the basic plane alone', () => {
    const changed: string[] = []
    for (let code = 0; code <= 0xffff; code++) if (!isHidden(code) && printableText(String.fromCharCode(code)) !== String.fromCharCode(code)) changed.push(code.toString(16))
    expect(changed).toEqual([])
  })

  it.each(sharedHidden.textCases.map((one, index) => [index, one] as const))('says case %i of the shared file as the file does', (_index, one) => {
    expect(printableText(one.text)).toBe(one.printable)
  })

  it('has the cases the file is for: a line that stays, a hidden character that changes, and the joiners and scripts that stay', () => {
    const cases = sharedHidden.textCases
    expect(cases.some((one) => one.text.includes('\n') && one.printable.includes('\n'))).toBe(true)
    expect(cases.some((one) => one.printable !== one.text)).toBe(true)
    expect(cases.some((one) => one.text.includes(String.fromCodePoint(0x200d)) && one.printable === one.text)).toBe(true)
    expect(cases.some((one) => /\p{Script=Arabic}/u.test(one.text) && one.printable === one.text)).toBe(true)
  })

  it('is the label’s way of printing with the line breaks kept, and nothing more', () => {
    const rlo = String.fromCodePoint(0x202e)
    expect(printableText(`a${rlo}b\nc\td`)).toBe('a?b\nc\td')
    expect(printable('a\nb\tc')).toBe('a?b?c')
  })
})

describe('the texts of an answer, made printable wherever they lie', () => {
  const rlo = String.fromCodePoint(0x202e)
  const dirty = `Ship${rlo} it\nnow`
  const shown = 'Ship? it\nnow'

  it('turns a title and a summary as labels (a line break is a mark) and a context, an option’s text and detail, and a thread’s text as texts (the lines stay)', () => {
    const answer = {
      view: {
        requests: [{ id: 'r', title: dirty, context: dirty, document: dirty, options: [{ key: 'a', text: dirty, detail: dirty }], thread: [{ from: 'x', text: dirty }] }],
        work: [{ ref: 'a#1', title: dirty }],
        people: [{ id: 'b', label: dirty, status: dirty }],
      },
      events: [{ kind: 'message', summary: dirty }, { kind: 'itemAdded', title: dirty }],
    }
    expect(shownOf(labelsMadePrintable(answer))).toEqual({
      view: {
        requests: [{ id: 'r', title: 'Ship? it?now', context: shown, document: shown, options: [{ key: 'a', text: shown, detail: shown }], thread: [{ from: 'x', text: shown }] }],
        work: [{ ref: 'a#1', title: 'Ship? it?now' }],
        people: [{ id: 'b', label: 'Ship? it?now', status: 'Ship? it?now' }],
      },
      events: [{ kind: 'message', summary: 'Ship? it?now' }, { kind: 'itemAdded', title: 'Ship? it?now' }],
    })
  })

  it('leaves a name alone (an id is looked for by it), and a text that is not text', () => {
    const answer = { from: dirty, to: dirty, id: dirty, text: 7, title: null, summary: ['x'] }
    expect(labelsMadePrintable(answer)).toEqual(answer)
  })
})

describe('what the node gave, kept for what is sent back', () => {
  const dirty = `Ship${String.fromCodePoint(0x202e)} it\nnow`

  it('keeps what a label and a text were before they were made printable, and gives them back by key', () => {
    const request = (labelsMadePrintable([{ title: dirty, context: dirty, id: 'r-1' }]) as Array<{ title: string; context: string }>)[0] as { title: string; context: string }
    expect(request.title).toBe('Ship? it?now')
    expect(asGiven(request, 'title')).toBe(dirty)
    expect(asGiven(request, 'context')).toBe(dirty)
  })

  it('has nothing to give back for what was not changed, and says the value as it is then', () => {
    const request = (labelsMadePrintable([{ title: 'Plain', id: 'r-1' }]) as Array<{ title: string }>)[0] as { title: string }
    expect(request).not.toHaveProperty('asGiven')
    expect(asGiven(request, 'title')).toBe('Plain')
    expect(asGiven(request, 'missing')).toBeUndefined()
  })
})
