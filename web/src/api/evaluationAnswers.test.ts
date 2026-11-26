import { readEffort, readEvaluation } from './evaluationAnswers'

const tokens = { input: 1, output: 2, cacheRead: 3, cacheWrite: 4 }

describe('readEffort', () => {
  it('counts a work type the node leaves out as 0, in the totals and in a day', () => {
    const effort = readEffort({
      org: 'acme',
      days: 14,
      generatedAt: '2026-11-26T09:00:00Z',
      teams: [{ team: 'app', byType: { newFeature: { costUsd: 5, tokens } }, byDay: [{ date: '2026-11-26', costUsd: { newFeature: 5 } }] }],
    })
    const team = effort.teams[0]
    expect(team?.byType.incident).toEqual({ costUsd: 0, tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } })
    expect(team?.byType.newFeature.costUsd).toBe(5)
    expect(team?.byDay[0]?.costUsd).toEqual({ newFeature: 5, improvement: 0, maintenance: 0, debt: 0, incident: 0, unclassified: 0 })
  })

  it('has no teams and no unavailable teams when the answer lists none', () => {
    const effort = readEffort({ org: 'acme', days: 14, generatedAt: '2026-11-26T09:00:00Z' })
    expect(effort.teams).toEqual([])
    expect(effort.unavailable).toEqual([])
  })

  it.each([
    ['a negative cost', { newFeature: { costUsd: -1, tokens } }],
    ['a cost that is not a number', { newFeature: { costUsd: '5', tokens } }],
    ['a cost that is not finite', { newFeature: { costUsd: Number.POSITIVE_INFINITY, tokens } }],
  ])('refuses the whole answer for %s', (_name, byType) => {
    expect(() => readEffort({ org: 'acme', days: 14, generatedAt: '2026-11-26T09:00:00Z', teams: [{ team: 'app', byType, byDay: [] }] })).toThrow(/bad/)
  })

  it('refuses a day whose cost is negative', () => {
    expect(() => readEffort({ org: 'acme', days: 14, generatedAt: 't', teams: [{ team: 'app', byType: {}, byDay: [{ date: '2026-11-26', costUsd: { debt: -3 } }] }] })).toThrow(/bad/)
  })

  it('reads the teams that could not answer, and the period the node names', () => {
    const effort = readEffort({ org: 'acme', days: 14, generatedAt: 't', from: '2026-11-13', until: '2026-11-26', unavailable: [{ team: 'site', reason: 'Its node did not answer.' }] })
    expect(effort.unavailable).toEqual([{ team: 'site', reason: 'Its node did not answer.' }])
    expect([effort.from, effort.until]).toEqual(['2026-11-13', '2026-11-26'])
  })
})

describe('readEvaluation', () => {
  it('counts a pull request type the node leaves out as 0, in the summary and in a day', () => {
    const evaluation = readEvaluation({
      org: 'acme',
      days: 14,
      generatedAt: '2026-11-26T09:00:00Z',
      teams: [{ team: 'app', summary: { merged: { feat: 3 }, mainRedSeconds: 60 }, byDay: [{ date: '2026-11-26', merged: { fix: 2 } }] }],
    })
    const team = evaluation.teams[0]
    expect(team?.summary.merged).toEqual({ feat: 3, fix: 0, refactor: 0, docs: 0, test: 0, chore: 0, other: 0 })
    expect(team?.summary.prLeadMedianSeconds).toBeNull()
    expect(team?.byDay[0]?.merged.fix).toBe(2)
    expect(team?.byDay[0]?.prLeadSeconds).toEqual([])
  })

  it('refuses a negative count and a lead time that is not a number', () => {
    const answer = (day: unknown) => ({ org: 'acme', days: 1, generatedAt: 't', teams: [{ team: 'app', summary: {}, byDay: [day] }] })
    expect(() => readEvaluation(answer({ date: '2026-11-26', merged: { feat: -1 } }))).toThrow(/bad/)
    expect(() => readEvaluation(answer({ date: '2026-11-26', prLeadSeconds: ['soon'] }))).toThrow(/bad/)
  })
})
