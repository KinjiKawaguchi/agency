import type { ActivityEvent } from '../api/activityApi'
import { edgesOf, guestsOf, keyOf, motionOf, placeAgents, resolverOf, settle, step, type Agent } from './graph'
import type { TaggedEvent } from './stream'

const SIZE = { width: 800, height: 600 }
const agent = (id: string, team = 'app'): Agent => ({ id, team, org: 'acme', label: id })
const AGENTS = [agent('lead'), agent('backend'), agent('frontend'), agent('docs'), agent('designer', 'site'), agent('builder', 'site')]
const NOW = Date.parse('2026-11-26T10:00:00Z')
const resolve = resolverOf(AGENTS)
const message = (from: string, to: string, secondsAgo: number, team = 'app'): TaggedEvent => ({
  org: 'acme',
  team,
  event: { id: `m-${from}-${to}-${secondsAgo}`, at: new Date(NOW - secondsAgo * 1000).toISOString(), kind: 'message', from, to, summary: '' } satisfies ActivityEvent,
})

describe('edgesOf', () => {
  it('joins two agents that wrote to each other, in either direction, counting newer messages more', () => {
    const edges = edgesOf([message('lead', 'backend', 10), message('backend', 'lead', 20), message('lead', 'docs', 200)], { now: NOW, resolve })
    expect(edges.map((edge) => [edge.a, edge.b])).toEqual([['acme/app/backend', 'acme/app/lead'], ['acme/app/docs', 'acme/app/lead']])
    expect(edges[0]!.weight).toBeGreaterThan(1.8)
    expect(edges[1]!.weight).toBeLessThan(edges[0]!.weight)
    expect(edges[0]!.lastAt).toBe(NOW - 10_000)
  })

  it('counts a message for half after the half-life, and for nothing after the window or before it was sent', () => {
    const [half] = edgesOf([message('lead', 'backend', 180)], { now: NOW, resolve })
    expect(half!.weight).toBeCloseTo(0.5)
    expect(edgesOf([message('lead', 'backend', 601)], { now: NOW, resolve })).toEqual([])
    expect(edgesOf([message('lead', 'backend', -5)], { now: NOW, resolve })).toEqual([])
  })

  it('keeps two teams’ agents of the same name apart, and ignores a message to oneself and what is not a message', () => {
    const twins = resolverOf([...AGENTS, agent('lead', 'site'), agent('backend', 'site')])
    const edges = edgesOf([message('lead', 'backend', 5), message('lead', 'backend', 5, 'site'), message('lead', 'lead', 5)], { now: NOW, resolve: twins })
    expect(edges).toHaveLength(2)
    const merged: TaggedEvent = { org: 'acme', team: 'app', event: { id: 'b-1', at: new Date(NOW).toISOString(), kind: 'prMerged', ref: 'app#1', title: 'x' } }
    expect(edgesOf([merged], { now: NOW, resolve })).toEqual([])
  })
})

describe('who a name in a message is', () => {
  it('is a participant of the team the feed is of, else the one agent of that name in the org, else a guest', () => {
    expect(resolve('acme', 'app', 'lead')).toBe('acme/app/lead')
    // A message in the feed of `site` that names an agent of `app`: the lead is not on `site`, but is the one `lead` of the org.
    expect(resolve('acme', 'site', 'lead')).toBe('acme/app/lead')
    expect(resolve('acme', 'app', 'AgencyLead')).toBe('acme/-/AgencyLead')
    expect(resolverOf([agent('lead'), agent('lead', 'site')])('acme', 'other', 'lead')).toBe('acme/-/lead')
  })

  it('joins a message across teams to a line between the two agents, and draws the one who is in no team as a guest', () => {
    const events = [message('designer', 'lead', 5, 'site'), message('lead', 'AgencyLead', 6)]
    const edges = edgesOf(events, { now: NOW, resolve })
    expect(edges.map((edge) => [edge.a, edge.b])).toEqual([['acme/-/AgencyLead', 'acme/app/lead'], ['acme/app/lead', 'acme/site/designer']])
    expect(guestsOf(events, resolve, { now: NOW }).shown.map((guest) => [keyOf(guest), guest.label, guest.team])).toEqual([['acme/-/AgencyLead', 'AgencyLead', '-']])
  })

  it('draws at most a dozen guests, the busiest first, and says how many there are', () => {
    const many = Array.from({ length: 20 }, (_, index) => message('lead', `Guest${String(index).padStart(2, '0')}`, 5 + index))
    // Guest07 writes three more times, so it comes first whatever its name.
    const busy = [message('lead', 'Guest07', 1), message('lead', 'Guest07', 2), message('lead', 'Guest07', 3)]
    const { shown, total } = guestsOf([...many, ...busy], resolve, { now: NOW })
    expect(total).toBe(20)
    expect(shown).toHaveLength(12)
    expect(shown[0]!.label).toBe('Guest07')
  })

  it('draws a guest only while a recent message names them', () => {
    expect(guestsOf([message('lead', 'AgencyLead', 700)], resolve, { now: NOW })).toEqual({ shown: [], total: 0 })
  })
})

describe('the layout', () => {
  it('puts each agent in a place of its own, in the picture, and is the same for the same agents', () => {
    const points = placeAgents(AGENTS, [], SIZE)
    expect(points.map((point) => point.key)).toEqual(AGENTS.map(keyOf))
    expect(new Set(points.map((point) => `${point.x.toFixed(2)},${point.y.toFixed(2)}`)).size).toBe(AGENTS.length)
    expect(placeAgents(AGENTS, [], SIZE)).toEqual(points)
  })

  it('keeps the agents it has placed where they are, and starts a new one near its team', () => {
    const first = placeAgents(AGENTS.slice(0, 4), [], SIZE)
    const next = placeAgents(AGENTS, first, SIZE)
    expect(next.slice(0, 4)).toEqual(first)
    const designer = next.find((point) => point.key === 'acme/site/designer')!
    const builder = next.find((point) => point.key === 'acme/site/builder')!
    expect(Math.hypot(designer.x - builder.x, designer.y - builder.y)).toBeLessThan(SIZE.width * 0.2)
  })

  it('settles inside the picture, with the agents apart, the teams gathered, and those who write to each other nearer', () => {
    const edges = edgesOf([message('lead', 'backend', 5), message('lead', 'backend', 15), message('lead', 'backend', 25)], { now: NOW, resolve })
    const settled = settle(placeAgents(AGENTS, [], SIZE), { agents: AGENTS, edges, size: SIZE })
    const at = (name: string, team = 'app') => settled.find((point) => point.key === `acme/${team}/${name}`)!
    for (const point of settled) {
      expect(point.x).toBeGreaterThan(0)
      expect(point.x).toBeLessThan(SIZE.width)
      expect(point.y).toBeGreaterThan(0)
      expect(point.y).toBeLessThan(SIZE.height)
    }
    const gap = (a: { x: number; y: number }, b: { x: number; y: number }) => Math.hypot(a.x - b.x, a.y - b.y)
    for (let i = 0; i < settled.length; i += 1) for (let j = i + 1; j < settled.length; j += 1) expect(gap(settled[i]!, settled[j]!)).toBeGreaterThan(20)
    expect(gap(at('lead'), at('backend'))).toBeLessThan(gap(at('lead'), at('docs')))
    expect(gap(at('lead'), at('frontend'))).toBeLessThan(gap(at('lead'), at('builder', 'site')))
  })

  it('comes to rest, and does not change what it is given', () => {
    const start = placeAgents(AGENTS, [], SIZE)
    const frozen = JSON.stringify(start)
    const settled = settle(start, { agents: AGENTS, edges: [], size: SIZE })
    expect(JSON.stringify(start)).toBe(frozen)
    expect(motionOf(settled)).toBeLessThan(0.5)
    expect(motionOf(step(settled, { agents: AGENTS, edges: [], size: SIZE }, 0.02))).toBeLessThan(0.5)
  })

  it('draws a single agent, and none, without failing', () => {
    expect(settle(placeAgents([AGENTS[0]!], [], SIZE), { agents: [AGENTS[0]!], edges: [], size: SIZE })).toHaveLength(1)
    expect(settle([], { agents: [], edges: [], size: SIZE })).toEqual([])
  })
})
