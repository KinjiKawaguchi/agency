import { readEffort, readEvaluation } from './evaluationAnswers'

const tokens = { input: 1, output: 2, cacheRead: 3, cacheWrite: 4 }
const period = { from: '2026-11-13', until: '2026-11-26' }
const header = { org: 'acme', days: 14, generatedAt: '2026-11-26T09:00:00Z', ...period }

describe('readEffort', () => {
  it('counts a work type the node leaves out as 0, in the totals and in a day', () => {
    const effort = readEffort({
      ...header,
      teams: [{ team: 'app', ...period, byType: { newFeature: { costUsd: 5, tokens } }, byDay: [{ date: '2026-11-26', costUsd: { newFeature: 5 } }] }],
    })
    const team = effort.teams[0]
    expect(team?.byType.incident).toEqual({ costUsd: 0, tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } })
    expect(team?.byType.newFeature.costUsd).toBe(5)
    expect(team?.byDay[0]?.costUsd).toEqual({ newFeature: 5, improvement: 0, maintenance: 0, debt: 0, incident: 0, unclassified: 0 })
  })

  it('has no teams and no unavailable teams when the answer lists none', () => {
    const effort = readEffort(header)
    expect(effort.teams).toEqual([])
    expect(effort.unavailable).toEqual([])
  })

  it.each([
    ['a negative cost', { newFeature: { costUsd: -1, tokens } }],
    ['a cost that is not a number', { newFeature: { costUsd: '5', tokens } }],
    ['a cost that is not finite', { newFeature: { costUsd: Number.POSITIVE_INFINITY, tokens } }],
  ])('refuses the whole answer for %s', (_name, byType) => {
    expect(() => readEffort({ ...header, teams: [{ team: 'app', ...period, byType, byDay: [] }] })).toThrow(/bad/)
  })

  it('refuses a day whose cost is negative', () => {
    expect(() => readEffort({ ...header, teams: [{ team: 'app', ...period, byType: {}, byDay: [{ date: '2026-11-26', costUsd: { debt: -3 } }] }] })).toThrow(/bad/)
  })

  it('reads the teams that could not answer, and the period of the report and of each team', () => {
    const effort = readEffort({ ...header, teams: [{ team: 'app', from: '2026-11-14', until: '2026-11-27', byType: {}, byDay: [] }], unavailable: [{ team: 'site', reason: 'Its node did not answer.' }] })
    expect(effort.unavailable).toEqual([{ team: 'site', reason: 'Its node did not answer.' }])
    expect([effort.from, effort.until]).toEqual(['2026-11-13', '2026-11-26'])
    expect([effort.teams[0]?.from, effort.teams[0]?.until]).toEqual(['2026-11-14', '2026-11-27'])
  })

  it('refuses a team that names no period, since its days are drawn against it', () => {
    expect(() => readEffort({ ...header, teams: [{ team: 'app', byType: {}, byDay: [] }] })).toThrow(/bad last day|bad first day/)
  })
})

describe('readEvaluation: participants', () => {
  const team = { team: 'app', ...period, summary: { merged: { feat: 1 } }, byDay: [] }
  const one = { name: 'backend', busySeconds: 60, waitingSeconds: 30, idleSeconds: 10, costUsd: 1.5, tokens, merged: 1, costPerMergedPrUsd: 1.5 }

  it('reads the participants and the pull requests nobody is named for, and keeps what the node leaves out left out', () => {
    const read = readEvaluation({ ...header, teams: [{ ...team, participants: [one, { ...one, name: 'lead', merged: 0, costPerMergedPrUsd: undefined }], unattributedMerged: 2 }] }).teams[0]
    expect(read?.participants?.map((p) => p.name)).toEqual(['backend', 'lead'])
    expect(read?.participants?.[0]).toEqual(one)
    expect(read?.participants?.[1]).not.toHaveProperty('costPerMergedPrUsd')
    expect(read?.unattributedMerged).toBe(2)
  })

  it('has none when the node gives none: for anyone but the team’s owner, or from a runtime that does not report them', () => {
    const read = readEvaluation({ ...header, teams: [team] }).teams[0]
    expect(read).not.toHaveProperty('participants')
    expect(read).not.toHaveProperty('unattributedMerged')
  })

  it.each([
    ['a negative time', { ...one, busySeconds: -1 }],
    ['a cost that is not a number', { ...one, costUsd: '1' }],
    ['a participant with no name', { ...one, name: '' }],
    ['tokens that are not numbers', { ...one, tokens: { input: 'x' } }],
  ])('refuses the whole answer for %s', (_name, bad) => {
    expect(() => readEvaluation({ ...header, teams: [{ ...team, participants: [bad] }] })).toThrow(/bad/)
  })
})

describe('readEvaluation', () => {
  it('counts a pull request type the node leaves out as 0, in the summary and in a day', () => {
    const evaluation = readEvaluation({
      ...header,
      teams: [{ team: 'app', ...period, summary: { merged: { feat: 3 }, mainRedSeconds: 60 }, byDay: [{ date: '2026-11-26', merged: { fix: 2 } }] }],
    })
    const team = evaluation.teams[0]
    expect(team?.summary.merged).toEqual({ feat: 3, fix: 0, refactor: 0, docs: 0, test: 0, chore: 0, other: 0 })
    expect(team?.byDay[0]?.merged.fix).toBe(2)
    expect(team?.byDay[0]?.prLeadSeconds).toEqual([])
  })

  it('leaves out what the node leaves out, and reads a null as left out, never as 0', () => {
    const summary = readEvaluation({ ...header, teams: [{ team: 'app', ...period, summary: { merged: {}, prLeadMedianSeconds: 90, issueLeadMedianSeconds: null }, byDay: [] }] }).teams[0]?.summary
    expect(summary?.prLeadMedianSeconds).toBe(90)
    expect(summary && 'issueLeadMedianSeconds' in summary).toBe(false)
    expect(summary && 'costPerMergedPrUsd' in summary).toBe(false)
  })

  it('refuses a negative count and a lead time that is not a number', () => {
    const answer = (day: unknown) => ({ ...header, teams: [{ team: 'app', ...period, summary: {}, byDay: [day] }] })
    expect(() => readEvaluation(answer({ date: '2026-11-26', merged: { feat: -1 } }))).toThrow(/bad/)
    expect(() => readEvaluation(answer({ date: '2026-11-26', prLeadSeconds: ['soon'] }))).toThrow(/bad/)
  })

  it('refuses an answer that names no period', () => {
    expect(() => readEvaluation({ org: 'acme', days: 14, generatedAt: 't', teams: [] })).toThrow(/bad/)
  })
})
