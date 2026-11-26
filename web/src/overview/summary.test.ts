import { sampleTeamView, type PersonStatus, type TeamView } from '../api/teamview'
import { teamOf } from '../test/views'
import { orderTeams, orgRank, rankOf, summarize } from './summary'
import type { TeamState } from '../api/queries'

const person = (id: string, state: PersonStatus['state'], status = ''): PersonStatus => ({ id, label: id, kind: 'agent', role: 'member', state, status, holding: [] })
const viewOf = (overrides: Partial<TeamView>): TeamView => ({ ...sampleTeamView, requests: [], work: [], people: [], ...overrides })
const ready = (view: TeamView, mine?: readonly string[]): TeamState => ({ status: 'ready', team: teamOf(view, 'owner', mine ?? []) })

describe('summarize', () => {
  it('counts what the node lists as the viewer’s, and calls the team “mine” however busy it is', () => {
    const view = viewOf({ people: [person('a', 'working')], requests: sampleTeamView.requests })
    const summary = summarize(teamOf(view, 'owner', ['r-1', 'r-2']))
    expect(summary).toMatchObject({ attention: 'mine', mine: 2 })
  })

  it('leaves the viewer out of the participants and counts who is working', () => {
    const view = viewOf({ people: [person('owner', 'unknown'), person('a', 'working'), person('b', 'idle'), person('c', 'working')] })
    expect(summarize(teamOf(view, 'owner', []))).toMatchObject({ participants: 3, working: 2, attention: 'active' })
  })

  it('is stuck when work is blocked, and says why in the reasons the team gives, each counted', () => {
    const view = viewOf({
      people: [person('a', 'working')],
      work: [
        { ref: 'x#1', title: 'one', status: 'next', blocked: { reason: 'needsDecision', since: '2026-11-20' } },
        { ref: 'x#2', title: 'two', status: 'next', blocked: { reason: 'needsDecision', since: '2026-11-21' } },
        { ref: 'x#3', title: 'three', status: 'next', blocked: { reason: 'external', since: '2026-11-22' } },
        { ref: 'x#4', title: 'four', status: 'next' },
      ],
    })
    expect(summarize(teamOf(view, 'owner', []))).toMatchObject({
      attention: 'stuck',
      blockedCount: 3,
      blocked: [
        { reason: 'needsDecision', count: 2 },
        { reason: 'external', count: 1 },
      ],
    })
  })

  it('is idle when nobody works and nothing is blocked, with what the quiet ones say, twice at most and once each', () => {
    const view = viewOf({ people: [person('a', 'idle', 'Nothing in Next'), person('b', 'away', 'Nothing in Next'), person('c', 'idle', 'Waiting for a build'), person('d', 'idle', 'Third line')] })
    expect(summarize(teamOf(view, 'owner', []))).toMatchObject({ attention: 'idle', quiet: ['Nothing in Next', 'Waiting for a build'] })
  })

  it('is not idle for a team with no participants at all', () => {
    expect(summarize(teamOf(viewOf({}), 'owner', [])).attention).toBe('active')
  })
})

describe('ordering', () => {
  const mine = ready(viewOf({ people: [person('a', 'working')] }), ['r-1'])
  const stuck = ready(viewOf({ people: [person('a', 'working')], work: [{ ref: 'x#1', title: 'one', status: 'next', blocked: { reason: 'external', since: '2026-11-20' } }] }))
  const idle = ready(viewOf({ people: [person('a', 'idle', 'Nothing in Next')] }))
  const active = ready(viewOf({ people: [person('a', 'working')] }))
  const failed: TeamState = { status: 'failed', at: 0, error: new Error('no') }
  const loading: TeamState = { status: 'loading' }

  it('puts the teams that wait on the viewer first, then stuck, a team that cannot be read, idle, the rest, and what still loads', () => {
    const teams = [loading, active, idle, failed, stuck, mine].map((state, index) => ({ label: `t${index}`, state }))
    expect(orderTeams(teams).map((team) => team.state.status === 'ready' ? summarize(team.state.team).attention : team.state.status)).toEqual(['mine', 'stuck', 'failed', 'idle', 'active', 'loading'])
  })

  it('puts the team with more waiting on the viewer first, then by name', () => {
    const two = ready(viewOf({}), ['r-1', 'r-2'])
    const one = ready(viewOf({}), ['r-3'])
    expect(orderTeams([{ label: 'b', state: one }, { label: 'a', state: one }, { label: 'z', state: two }]).map((team) => team.label)).toEqual(['z', 'a', 'b'])
  })

  it('ranks an org by its most pressing team', () => {
    expect(orgRank([{ label: 'a', state: active }, { label: 'b', state: stuck }])).toBe(rankOf(stuck))
    expect(orgRank([])).toBeGreaterThan(rankOf(active))
  })
})
