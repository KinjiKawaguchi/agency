import { readBudget } from './budgetAnswers'
import { MalformedAnswer } from './evaluationAnswers'

const answer = { weekStart: '2026-11-23', team: { capUsd: 200, spentUsd: 163.2, state: 'warning' }, org: { capUsd: 500, spentUsd: 311, state: 'ok', partial: true } }

describe('reading the weekly budget answer', () => {
  it('keeps what the node says: the week, each part’s cap, spend and state, and `partial` only when it is true', () => {
    expect(readBudget(answer)).toEqual(answer)
    expect(readBudget({ ...answer, org: { ...answer.org, partial: false } }).org).toEqual({ capUsd: 500, spentUsd: 311, state: 'ok' })
  })

  it('leaves a cap out when the node leaves it out: a team with no plan', () => {
    expect(readBudget({ ...answer, team: { spentUsd: 5, state: 'ok' } }).team).toEqual({ spentUsd: 5, state: 'ok' })
  })

  it.each([
    ['no answer', undefined],
    ['a week that is not a date', { ...answer, weekStart: 'Monday' }],
    ['a team part that is missing', { ...answer, team: undefined }],
    ['a spend that is missing, which would read as nothing spent', { ...answer, team: { capUsd: 1, state: 'ok' } }],
    ['a spend that is negative', { ...answer, team: { ...answer.team, spentUsd: -1 } }],
    ['a spend that is text', { ...answer, org: { ...answer.org, spentUsd: '311' } }],
    ['a cap that is not a whole number', { ...answer, team: { ...answer.team, capUsd: 1.5 } }],
    ['a cap that is negative', { ...answer, team: { ...answer.team, capUsd: -1 } }],
    ['a state it does not know', { ...answer, team: { ...answer.team, state: 'critical' } }],
    ['no state', { ...answer, org: { capUsd: 5, spentUsd: 1 } }],
  ])('refuses the whole answer with %s', (_name, raw) => {
    expect(() => readBudget(raw)).toThrow(MalformedAnswer)
  })
})
