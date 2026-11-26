// Who writes to whom, as a picture: agents are points, and a line joins two agents that have written to each
// other lately. The layout is a small force simulation (points push each other apart, lines pull their ends
// together, each team gathers around a place of its own), written as pure steps so that a layout is the same
// from the same input and can be tested.
import type { TaggedEvent } from './stream'

export interface Agent {
  /** The participant's name, unique inside its team. */
  id: string
  team: string
  org: string
  label: string
}

export interface Point {
  key: string
  x: number
  y: number
  vx: number
  vy: number
}

export interface Edge {
  a: string
  b: string
  /** How much has passed between the two lately, newer counting more: 0 to a few. */
  weight: number
  /** When the latest message between them was sent, in ms. */
  lastAt: number
}

export interface Size {
  width: number
  height: number
}

export const keyOf = (agent: { org: string; team: string; id: string }): string => `${agent.org}/${agent.team}/${agent.id}`

/** A repeatable number in [0, 1) for a string, so that the same agents start in the same places. */
function noise(seed: string): number {
  let hash = 2166136261
  for (const character of seed) hash = Math.imul(hash ^ (character.codePointAt(0) ?? 0 /* stand-in-ok(index): a character code in a hash of a name */), 16777619) >>> 0
  return hash / 2 ** 32
}

/** The place each team gathers around: the teams spread on a circle around the middle. */
export function anchorsOf(teams: readonly string[], { width, height }: Size): Map<string, { x: number; y: number }> {
  // An ellipse that follows the picture's shape, starting on the left, so that two teams sit side by side.
  const [radiusX, radiusY] = teams.length < 2 ? [0, 0] : [width * 0.27, height * 0.3]
  return new Map(
    teams.map((team, index) => {
      const angle = (index / Math.max(teams.length, 1)) * 2 * Math.PI + Math.PI
      return [team, { x: width / 2 + Math.cos(angle) * radiusX, y: height / 2 + Math.sin(angle) * radiusY }]
    }),
  )
}

const teamKey = (agent: Agent): string => `${agent.org}/${agent.team}`

/**
 * The points of these agents: those already placed stay where they are, and a new one starts near its team's
 * place, a little off to one side by its name so that two do not start on top of each other.
 */
export function placeAgents(agents: readonly Agent[], previous: readonly Point[], size: Size): Point[] {
  const known = new Map(previous.map((point) => [point.key, point]))
  const anchors = anchorsOf([...new Set(agents.map(teamKey))].toSorted(), size)
  return agents.map((agent) => {
    const key = keyOf(agent)
    const existing = known.get(key)
    if (existing !== undefined) return existing
    const anchor = anchors.get(teamKey(agent)) ?? { x: size.width / 2, y: size.height / 2 }
    const spread = Math.min(size.width, size.height) * 0.08
    return { key, x: anchor.x + (noise(`${key}:x`) - 0.5) * 2 * spread, y: anchor.y + (noise(`${key}:y`) - 0.5) * 2 * spread, vx: 0, vy: 0 }
  })
}

/** How much energy the layout has when it starts, and how much is left before it is taken for at rest. */
export const START_ENERGY = 1
export const REST_ENERGY = 0.02

interface Forces {
  agents: readonly Agent[]
  edges: readonly Edge[]
  size: Size
}

/** One step of the simulation; `energy` scales the forces and falls as the layout settles. */
export function step(points: readonly Point[], { agents, edges, size }: Forces, energy: number): Point[] {
  const place = new Map(points.map((point) => [point.key, point]))
  const anchors = anchorsOf([...new Set(agents.map(teamKey))].toSorted(), size)
  const teamOf = new Map(agents.map((agent) => [keyOf(agent), teamKey(agent)]))
  const reach = Math.min(size.width, size.height)
  const idealGap = reach * 0.2
  const force = new Map(points.map((point) => [point.key, { x: 0, y: 0 }]))
  const push = (key: string, x: number, y: number): void => {
    const total = force.get(key)
    if (total !== undefined) {
      total.x += x
      total.y += y
    }
  }
  for (let i = 0; i < points.length; i += 1) {
    for (let j = i + 1; j < points.length; j += 1) {
      const [p, q] = [points[i] as Point, points[j] as Point]
      const [dx, dy] = [p.x - q.x, p.y - q.y]
      const distance = Math.max(Math.hypot(dx, dy), 1)
      const push_ = (idealGap * idealGap) / distance / distance
      push(p.key, (dx / distance) * push_ * 1.0, (dy / distance) * push_ * 1.0)
      push(q.key, (-dx / distance) * push_ * 1.0, (-dy / distance) * push_ * 1.0)
    }
  }
  for (const edge of edges) {
    const [p, q] = [place.get(edge.a), place.get(edge.b)]
    if (p === undefined || q === undefined) continue
    const [dx, dy] = [q.x - p.x, q.y - p.y]
    const distance = Math.max(Math.hypot(dx, dy), 1)
    const pull = (distance - idealGap * 0.6) * 0.01 * Math.min(edge.weight, 3)
    push(p.key, (dx / distance) * pull, (dy / distance) * pull)
    push(q.key, (-dx / distance) * pull, (-dy / distance) * pull)
  }
  return points.map((point) => {
    const anchor = anchors.get(teamOf.get(point.key) ?? '' /* stand-in-ok(scale): where a point starts in the picture when its team has no anchor */) ?? { x: size.width / 2, y: size.height / 2 }
    const total = force.get(point.key) ?? { x: 0, y: 0 }
    // Each team gathers around its own place, and everything is held loosely to the middle.
    const gx = (anchor.x - point.x) * 0.0022 + (size.width / 2 - point.x) * 0.0006
    const gy = (anchor.y - point.y) * 0.0022 + (size.height / 2 - point.y) * 0.0006
    const vx = (point.vx + (total.x * reach * 0.002 + gx) * energy * 6) * 0.82
    const vy = (point.vy + (total.y * reach * 0.002 + gy) * energy * 6) * 0.82
    // A name is written under its point and may be wide, so the sides keep a margin for it, and the bottom one more.
    const sideMargin = Math.max(56, reach * 0.09)
    const [topMargin, bottomMargin] = [24, 48]
    return { key: point.key, x: Math.min(size.width - sideMargin, Math.max(sideMargin, point.x + vx)), y: Math.min(size.height - bottomMargin, Math.max(topMargin, point.y + vy)), vx, vy }
  })
}

/** Runs the simulation to rest, in one go: what a screen that does not animate shows, and where an animated one is headed. */
export function settle(points: readonly Point[], forces: Forces, ticks = 160): Point[] {
  let current = [...points]
  for (let tick = 0; tick < ticks; tick += 1) current = step(current, forces, Math.max(REST_ENERGY, START_ENERGY * (1 - tick / ticks)))
  return current
}

/** The most the points are still moving, which tells whether the layout is at rest. */
export const motionOf = (points: readonly Point[]): number => points.reduce((most, point) => Math.max(most, Math.hypot(point.vx, point.vy)), 0)

const key = (a: string, b: string): string => (a < b ? `${a}|${b}` : `${b}|${a}`)

/** The team of an agent that no team's view lists: someone who writes to or hears from a team's agents, such as the lead. */
export const GUEST_TEAM = '-'

/** Who a name in a message is, as the key of a point. */
export type Resolve = (org: string, team: string, name: string) => string

/**
 * A message names its sender and its recipient by participant name, and the other end is often not on the team the
 * feed is of (the lead, an agent of another team). A name is a participant of the team the feed is of when it is one;
 * otherwise it is the one agent of that name in the org, when there is exactly one; otherwise a guest, a point of its own.
 */
export function resolverOf(agents: readonly Agent[]): Resolve {
  const inTeam = new Set(agents.map(keyOf))
  const inOrg = new Map<string, string[]>()
  for (const agent of agents) inOrg.set(`${agent.org}/${agent.id}`, [...(inOrg.get(`${agent.org}/${agent.id}`) ?? [] /* stand-in-ok(index): a list under a key, started when the key is first seen */), keyOf(agent)])
  return (org, team, name) => {
    if (inTeam.has(keyOf({ org, team, id: name }))) return keyOf({ org, team, id: name })
    const named = inOrg.get(`${org}/${name}`) ?? [] /* stand-in-ok(index): a list under a key; none when no agent has the name */
    return named.length === 1 ? (named[0] as string) : keyOf({ org, team: GUEST_TEAM, id: name })
  }
}

/** The most guests drawn: the layout works on every pair, so the number of points is kept small however many names messages bring. */
export const MAX_GUESTS = 12

export interface Guests {
  /** The guests drawn: those named by the most recent messages. */
  shown: Agent[]
  /** How many there are in all. */
  total: number
}

/** The guests that appear in recent messages, the busiest first, as agents to draw. */
export function guestsOf(events: readonly TaggedEvent[], resolve: Resolve, { now, windowMs = 10 * 60_000, limit = MAX_GUESTS }: { now: number; windowMs?: number; limit?: number }): Guests {
  const found = new Map<string, Agent>()
  const counts = new Map<string, number>()
  for (const { org, team, event } of events) {
    if (event.kind !== 'message') continue
    const at = Date.parse(event.at)
    if (at > now || now - at > windowMs) continue
    for (const name of [event.from, event.to]) {
      const key = resolve(org, team, name)
      if (key === keyOf({ org, team: GUEST_TEAM, id: name })) {
        found.set(key, { id: name, label: name, team: GUEST_TEAM, org })
        counts.set(key, (counts.get(key) ?? 0 /* stand-in-ok(index): a count started when the key is first seen */) + 1)
      }
    }
  }
  const ordered = [...found.values()].toSorted((a, b) => (counts.get(keyOf(b)) ?? 0 /* stand-in-ok(index): a count that is 0 for a key never seen, to order the picture */) - (counts.get(keyOf(a)) ?? 0) || keyOf(a).localeCompare(keyOf(b)))
  return { shown: ordered.slice(0, limit), total: ordered.length }
}

export interface EdgeOptions {
  now: number
  resolve: Resolve
  /** Messages older than this count for nothing. */
  windowMs?: number
  /** A message counts for half after this long. */
  halfLifeMs?: number
}

/** The lines of the picture: two agents are joined by what has passed between them in the window, newer counting more. */
export function edgesOf(events: readonly TaggedEvent[], { now, resolve, windowMs = 10 * 60_000, halfLifeMs = 3 * 60_000 }: EdgeOptions): Edge[] {
  const found = new Map<string, Edge>()
  for (const { org, team, event } of events) {
    if (event.kind !== 'message') continue
    const at = Date.parse(event.at)
    if (at > now || now - at > windowMs) continue
    const [a, b] = [resolve(org, team, event.from), resolve(org, team, event.to)]
    if (a === b) continue
    const id = key(a, b)
    const before = found.get(id)
    const weight = 0.5 ** ((now - at) / halfLifeMs)
    found.set(id, { a: a < b ? a : b, b: a < b ? b : a, weight: (before?.weight ?? 0 /* stand-in-ok(index): a weight started when the pair is first seen */) + weight, lastAt: Math.max(before?.lastAt ?? 0, at) })
  }
  return [...found.values()].toSorted((x, y) => x.a.localeCompare(y.a) || x.b.localeCompare(y.b))
}
