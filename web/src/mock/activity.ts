// A made-up activity feed for the mock node: it never ends, and it is the same for a given moment, so that a
// client that asks for the last hour and then for what is new gets one consistent story. Time is cut into
// 5-second buckets, and each bucket decides, by a hash of its start, what happened in it. The agents are the
// team's own (from its view), who talk most through the lead; the work and the epics are the team's own too.
// Every name is fictional.
import type { Activity, ActivityEvent } from '../api/activityApi.ts'
import type { TeamView } from '../api/teamview.ts'

const BUCKET_MS = 5_000
/** The most events a read answers: what the harness would allow. */
export const MAX_EVENTS = 500

/** A stable number in [0, 1) for a string. */
function noise(seed: string): number {
  let hash = 2166136261
  for (const character of seed) hash = Math.imul(hash ^ (character.codePointAt(0) ?? 0), 16777619) >>> 0
  return hash / 2 ** 32
}

const shortId = (prefix: 'm' | 'b', seed: string): string => `${prefix}-${Math.floor(noise(`id:${seed}`) * 0xffffffff).toString(16).padStart(8, '0')}`

const MESSAGES = ['Which database for sessions?', 'Review please: store sessions', 'Tests are green', 'Need a decision on the export format', 'Starting the health check', 'Blocked on the schema', 'Docs for the session store are ready', 'Merging after the review', 'Can you take the mobile layout?', 'Nothing left in Next', 'Rebased on main', 'Found the cause of the slow query']
const TITLES = ['Cache the session lookup', 'Fix the date in the export', 'Show a spinner while saving', 'Add an index for sessions', 'Handle a missing email', 'Retry the failed upload', 'Explain the export in the docs', 'Rename the settings page', 'Close the idle connection', 'Check the token before saving']

const pick = <T,>(items: readonly T[], at: number): T => items[Math.min(items.length - 1, Math.floor(at * items.length))] as T

/** Who writes to whom: the lead is on one side of most messages, and the others also write among themselves. */
function pairOf(agents: readonly string[], seed: string): { from: string; to: string } | undefined {
  if (agents.length < 2) return undefined
  const lead = agents[0] as string
  const others = agents.slice(1)
  const withLead = noise(`${seed}:lead`) < 0.6
  const other = pick(others, noise(`${seed}:other`))
  if (withLead) return noise(`${seed}:dir`) < 0.5 ? { from: lead, to: other } : { from: other, to: lead }
  const second = pick(others.filter((one) => one !== other), noise(`${seed}:second`)) ?? lead
  return { from: other, to: second }
}

function eventsOfBucket(view: TeamView, start: number): ActivityEvent[] {
  const agents = view.people.filter((person) => person.kind === 'agent').toSorted((a, b) => (a.role === 'lead' ? -1 : 0) - (b.role === 'lead' ? -1 : 0)).map((person) => person.id)
  const seed = `${view.org}/${view.team}:${start}`
  const at = (name: string): string => new Date(start + Math.floor(noise(`${seed}:${name}:at`) * BUCKET_MS)).toISOString()
  const found: ActivityEvent[] = []
  const epic = pick(view.epics, noise(`${seed}:epic`))?.ref
  const withEpic = epic === undefined ? {} : { epic }
  const number = 100 + Math.floor(noise(`${seed}:n`) * 900)
  const ref = `${view.team}#${number}`
  const title = pick(TITLES, noise(`${seed}:title`))

  if (noise(`${seed}:message`) < 0.3) {
    const pair = pairOf(agents, seed)
    if (pair !== undefined) found.push({ id: shortId('m', `${seed}:message`), at: at('message'), kind: 'message', ...pair, summary: noise(`${seed}:blank`) < 0.1 ? '' : pick(MESSAGES, noise(`${seed}:summary`)) })
  }
  if (noise(`${seed}:added`) < 0.02) found.push({ id: shortId('b', `${seed}:added`), at: at('added'), kind: 'itemAdded', ref, title, ...withEpic })
  if (noise(`${seed}:status`) < 0.035) {
    const steps = [['next', 'inProgress'], ['inProgress', 'release'], ['release', 'done'], ['backlog', 'next']] as const
    const [from, to] = pick(steps, noise(`${seed}:step`))
    found.push({ id: shortId('b', `${seed}:status`), at: at('status'), kind: 'statusChanged', ref, title, from, to, ...withEpic })
  }
  if (noise(`${seed}:opened`) < 0.012) found.push({ id: shortId('b', `${seed}:opened`), at: at('opened'), kind: 'prOpened', ref: `${view.team}#${number + 1}`, title, ...withEpic })
  if (noise(`${seed}:merged`) < 0.009) {
    const leadSeconds = Math.round(600 + noise(`${seed}:lead-time`) * 6 * 3600)
    found.push({ id: shortId('b', `${seed}:merged`), at: at('merged'), kind: 'prMerged', ref: `${view.team}#${number + 2}`, title, ...withEpic, leadSeconds })
  }
  return found
}

/** The feed of a team since a time, as the node would answer: oldest first, strictly after `since`, cut at the limit. */
export function activityFor(view: TeamView, sinceMs: number, nowMs: number, limit: number = MAX_EVENTS): Activity {
  const first = Math.floor(sinceMs / BUCKET_MS) * BUCKET_MS
  const events: ActivityEvent[] = []
  for (let start = first; start <= nowMs; start += BUCKET_MS) events.push(...eventsOfBucket(view, start))
  const inWindow = events.filter((event) => Date.parse(event.at) > sinceMs && Date.parse(event.at) <= nowMs).toSorted((a, b) => Date.parse(a.at) - Date.parse(b.at))
  return { org: view.org, team: view.team, generatedAt: new Date(nowMs).toISOString(), events: inWindow.slice(0, limit), truncated: inWindow.length > limit }
}
