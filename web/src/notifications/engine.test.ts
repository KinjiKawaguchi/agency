import fixture from '../../../testdata/notifications.json' with { type: 'json' }
import { advance, START, type EngineState, type EventKind, type NoticeEvent, type Own, type Reading } from './engine'

// The same file the iPhone app's tests read, so the two clients cannot say different things are new.
interface Step {
  reading: Reading
  expected: Array<{ kind: EventKind; key: string }>
}
interface Case {
  name: string
  own: Own
  steps: Step[]
}
const cases = (fixture as { cases: Case[] }).cases

/** Runs a case from the baseline and answers the events of each step. */
function run(one: Case, change: (reading: Reading) => Reading = (reading) => reading): NoticeEvent[][] {
  let state: EngineState = START
  return one.steps.map((step) => {
    const next = advance(state, change(step.reading), one.own)
    state = next.state
    return next.events
  })
}

describe('what is new, by the shared cases', () => {
  for (const one of cases) {
    it(one.name, () => {
      expect(run(one)).toEqual(one.steps.map((step) => step.expected))
    })
  }

  it('has the cases the iPhone app reads', () => {
    expect(cases.length).toBeGreaterThanOrEqual(19)
  })
})

describe('the engine on its own', () => {
  it('does not change the state it was given', () => {
    const reading: Reading = { teams: [{ org: 'acme', team: 'app', ok: true, mine: ['r-1'], waitingOnViewer: [], blocked: [] }], devices: ['k1'], decisions: [{ id: 'd1', state: 'applied' }] }
    const own: Own = { deviceKey: 'own', canApprove: true, sentDecisionIds: [] }
    const first = advance(START, reading, own)
    expect(START.teams.size).toBe(0)
    expect(START.devices).toBeUndefined()
    const second = advance(first.state, { ...reading, teams: [{ ...reading.teams[0]!, mine: ['r-1', 'r-2'] }] }, own)
    expect(first.state.teams.get('acme/app')?.mine.has('r-2')).toBe(false)
    expect(second.events).toEqual([{ kind: 'request', key: 'request:acme/app/r-2' }])
  })

  it('orders keys by code point, not by UTF-16 unit', () => {
    // U+FF5E is above U+1F600 in UTF-16 order but below it by code point.
    const names = ['\u{1F600}', '～', 'a', 'B']
    const base: Reading = { teams: [{ org: 'acme', team: 'app', ok: true, mine: [], waitingOnViewer: [], blocked: [] }], devices: [], decisions: [] }
    const own: Own = { deviceKey: 'own', canApprove: true, sentDecisionIds: [] }
    const baseline = advance(START, base, own)
    const next = advance(baseline.state, { ...base, teams: [{ ...base.teams[0]!, mine: names }] }, own)
    // By code point: capitals before small letters, and the character of the first plane before one beyond it.
    expect(next.events.map((event) => event.key)).toEqual(['request:acme/app/B', 'request:acme/app/a', 'request:acme/app/～', 'request:acme/app/\u{1F600}'])
  })
})
