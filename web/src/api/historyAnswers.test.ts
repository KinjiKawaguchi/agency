import sample from '../../../testdata/history.sample.json' with { type: 'json' }
import { readHistory } from './historyAnswers'
import { MalformedAnswer } from './evaluationAnswers'

const entry = { version: 1, at: '2026-11-24T12:00:00Z', signature: 'valid', signedBy: 'root key 3f9c', follows: true, changes: ['create the org Acme'], isKept: true }
const bad = (change: string): string => change.replaceAll('X', String.fromCodePoint(0x202e))

describe('reading the history answer', () => {
  it('reads the node’s own sample as it is: a kept version, a gap, a version compared across it, and `more`', () => {
    expect(readHistory(sample)).toEqual(sample)
  })

  it('leaves out what the node leaves out, and `more` unless it is true', () => {
    expect(readHistory({ entries: [{ version: 2, changes: [], isKept: false }], more: false })).toEqual({ entries: [{ version: 2, changes: [], isKept: false }] })
  })

  it('makes each change and the signer printable: a character that reorders a line is a mark', () => {
    const read = readHistory({ entries: [{ ...entry, signedBy: bad('root keyX'), changes: [bad('add member boX (Bob)'), 'plain'] }] })
    expect(read.entries[0]?.changes).toEqual(['add member bo? (Bob)', 'plain'])
    expect(read.entries[0]?.signedBy).toBe('root key?')
  })

  it.each([
    ['no entries', {}],
    ['entries that are not a list', { entries: 'none' }],
    ['a version that is not a whole number', { entries: [{ ...entry, version: 1.5 }] }],
    ['a version of nothing', { entries: [{ ...entry, version: 0 }] }],
    ['a signature it cannot say', { entries: [{ ...entry, signature: 'maybe' }] }],
    ['no kept flag', { entries: [{ ...entry, isKept: undefined }] }],
    ['changes that are not a list', { entries: [{ ...entry, changes: 'create' }] }],
    ['a change that is not text', { entries: [{ ...entry, changes: [7] }] }],
    ['a change that is empty', { entries: [{ ...entry, changes: [''] }] }],
    ['a follows flag that is not one', { entries: [{ ...entry, follows: 'yes' }] }],
    ['a compared version that is not one', { entries: [{ ...entry, comparedWith: 0 }] }],
    ['a time that is empty', { entries: [{ ...entry, at: '' }] }],
    ['a `more` that is not a flag', { entries: [], more: 'yes' }],
  ])('refuses the whole answer with %s', (_name, raw) => {
    expect(() => readHistory(raw)).toThrow(MalformedAnswer)
  })
})
