import { sampleTeamView, type EpicForecast } from '../api/teamview'
import { blockedTotal, blockedWork, doneTime, epicSegments, inProgress, isDone, sortEpics, type Epic } from './work'

const epic = (change: Partial<EpicForecast>): EpicForecast => ({
  ref: 'app#1',
  title: 'Epic',
  total: 0,
  done: 0,
  doing: 0,
  remainingPoints: 0,
  blocked: [],
  basis: '',
  ...change,
})

describe('epicSegments', () => {
  it('splits the bar into done, doing, blocked, and not started', () => {
    const sample = sampleTeamView.epics[0]
    expect(sample && epicSegments(sample)).toEqual([
      { kind: 'done', count: 1 },
      { kind: 'doing', count: 1 },
      { kind: 'blocked', count: 2 },
    ])
  })

  it('drops empty segments and gives an epic with no children an empty bar', () => {
    expect(epicSegments(epic({ total: 0 }))).toEqual([])
    expect(epicSegments(epic({ total: 5, done: 2 }))).toEqual([
      { kind: 'done', count: 2 },
      { kind: 'rest', count: 3 },
    ])
  })

  it('never shows a negative remainder when the counts overlap', () => {
    expect(epicSegments(epic({ total: 2, done: 2, doing: 1 })).some((segment) => segment.kind === 'rest')).toBe(false)
  })
})

describe('blockedTotal', () => {
  it('adds up the reasons', () => {
    expect(blockedTotal(epic({ blocked: [{ reason: 'external', count: 2 }, { reason: 'needsDecision', count: 1 }] }))).toBe(3)
  })
})

describe('sortEpics', () => {
  it('puts epics with work in progress first, then the furthest along', () => {
    const sorted = sortEpics([epic({ ref: 'a', total: 4, done: 1 }), epic({ ref: 'b', total: 4, done: 3 }), epic({ ref: 'c', total: 4, doing: 1 })])
    expect(sorted.map((one) => one.ref)).toEqual(['c', 'b', 'a'])
  })
})

describe('work lists', () => {
  it('separates what is in progress from what is blocked', () => {
    expect(inProgress(sampleTeamView.work).map((item) => item.ref)).toEqual(['app#12'])
    expect(blockedWork(sampleTeamView.work).map((item) => item.ref)).toEqual(['app#14', 'app#15'])
  })
})

describe('finished epics', () => {
  const finished = (ref: string, doneAt?: string): Epic => ({ ...epic({ ref, total: 4, done: 4 }), ...(doneAt === undefined ? {} : { doneAt }) })

  it('are the ones with every child done and none in progress, and not an epic with no children', () => {
    expect(isDone(epic({ total: 4, done: 4 }))).toBe(true)
    expect(isDone(epic({ total: 4, done: 3 }))).toBe(false)
    expect(isDone(epic({ total: 4, done: 4, doing: 1 }))).toBe(false)
    expect(isDone(epic({ total: 0, done: 0 }))).toBe(false)
  })

  it('come after the epics still open, the latest finished first', () => {
    const sorted = sortEpics([finished('old', '2026-11-01T00:00:00Z'), epic({ ref: 'open', total: 4, done: 1 }), finished('new', '2026-11-20T00:00:00Z'), epic({ ref: 'going', total: 4, doing: 2 })])
    expect(sorted.map((one) => one.ref)).toEqual(['going', 'open', 'new', 'old'])
  })

  it('read a time that is not one as no time: the order is still settled and nothing is shown as an invalid date', () => {
    const sorted = sortEpics([finished('bad', 'yesterday'), finished('good', '2026-11-20T00:00:00Z'), finished('none')])
    expect(sorted.map((one) => one.ref)).toEqual(['good', 'bad', 'none'])
    expect(doneTime(finished('bad', 'yesterday'))).toBeUndefined()
    expect(doneTime(finished('good', '2026-11-20T00:00:00Z'))).toBe(Date.parse('2026-11-20T00:00:00Z'))
  })

  it('keep their place among themselves when none says when it was finished', () => {
    expect(sortEpics([finished('a'), finished('b')]).map((one) => one.ref)).toEqual(['a', 'b'])
  })
})
