import sample from '../../../testdata/client/org-members.json' with { type: 'json' }
import { readMembers } from './membersAnswers'
import { MalformedAnswer } from './evaluationAnswers'

const dirty = (text: string): string => text.replaceAll('X', String.fromCodePoint(0x202e))

describe('reading the members answer', () => {
  it('reads the node’s own sample as it is', () => {
    expect(readMembers(sample)).toEqual(sample)
  })

  it('makes what to ask a seat or an external party about printable, each topic of the list: a character that reorders a line is a mark', () => {
    const answer = readMembers({
      org: 'acme',
      members: [],
      seats: [{ id: 'dan', label: 'Dan', askAbout: [dirty('invoicesX'), 'plain'], answeredBy: [] }],
      externals: [{ id: 'erin', label: 'Erin', askAbout: [dirty('XcontractsX')], answeredBy: [] }],
    })
    expect(answer.seats[0]?.askAbout).toEqual(['invoices?', 'plain'])
    expect(answer.externals[0]?.askAbout).toEqual(['?contracts?'])
  })

  it.each([
    ['a topic that is not text', { askAbout: [7] }],
    ['a topic that is empty', { askAbout: [''] }],
    ['topics that are not a list', { askAbout: 'invoices' }],
  ])('refuses the whole answer with %s', (_name, party) => {
    expect(() => readMembers({ org: 'acme', members: [], seats: [{ id: 'dan', label: 'Dan', answeredBy: [], ...party }], externals: [] })).toThrow(MalformedAnswer)
  })
})
