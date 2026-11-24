// Mock org structure, funding, and token use for the mock node. Every name here is fictional.
// The shapes are the ones the real node will serve; the numbers are made up.
import type { FundingSource, OrgDefinition, UsageReport } from '../api/clientApi.ts'
import type { TeamView } from '../api/teamview.ts'

const MINUTE = 60_000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

export const ORG_DEFINITION: OrgDefinition = {
  org: 'acme',
  label: 'Acme',
  version: 3,
  departments: [
    { id: 'engineering', label: 'Engineering', head: 'owner' },
    { id: 'platform', label: 'Platform', parent: 'engineering', head: 'bob' },
    { id: 'design', label: 'Design', head: 'carol' },
  ],
  people: [
    { id: 'owner', label: 'Alice', department: 'engineering', role: 'member' },
    { id: 'bob', label: 'Bob', department: 'platform', role: 'member' },
    { id: 'carol', label: 'Carol', department: 'design', role: 'answerer' },
  ],
  teams: [
    { id: 'app', label: 'App', department: 'engineering' },
    { id: 'site', label: 'Site', department: 'design' },
  ],
}

/** A second, quiet team, so the org chart and the people screen show more than one. */
export function siteTeamView(base: TeamView): TeamView {
  return {
    ...base,
    team: 'site',
    label: 'Site',
    people: [
      { id: 'designer', label: 'Designer', kind: 'agent', role: 'lead', state: 'working', since: base.generatedAt, status: 'Drafting the pricing page', holding: ['site#3'] },
      { id: 'assets', label: 'Assets', kind: 'agent', role: 'member', state: 'idle', status: 'Idle; nothing in Next', holding: [] },
      { id: 'owner', label: 'Alice', kind: 'human', role: 'member', state: 'unknown', status: 'Nothing is waiting for an answer', holding: [] },
    ],
    requests: [],
    work: [{ ref: 'site#3', title: 'Redesign the pricing page', status: 'inProgress', assignee: 'designer', points: 3 }],
    epics: [],
  }
}

export function fundingSources(now: Date): FundingSource[] {
  const at = (ms: number) => new Date(now.getTime() + ms).toISOString()
  return [
    {
      id: 'personal',
      label: 'Personal plan',
      kind: 'subscription',
      sevenDay: { utilization: 62, resetsAt: at(3 * DAY), projectedAtReset: 96, status: 'ok' },
      fiveHour: { utilization: 74, resetsAt: at(2 * HOUR), projectedAtReset: 118, hitAt: at(80 * MINUTE), status: 'overrun' },
      teams: [{ org: 'acme', team: 'app' }],
    },
    {
      id: 'lab',
      label: 'Lab plan',
      kind: 'subscription',
      sevenDay: { utilization: 21, resetsAt: at(5 * DAY), projectedAtReset: 44, status: 'waste' },
      teams: [{ org: 'acme', team: 'site' }],
    },
  ]
}

/** A stable number in [0, 1) for a string, so the same inputs always draw the same chart. */
function noise(seed: string): number {
  let hash = 2166136261
  for (const char of seed) hash = Math.imul(hash ^ (char.codePointAt(0) ?? 0), 16777619) >>> 0
  return hash / 2 ** 32
}

const DAILY_BASE_USD: Record<string, number> = { lead: 14, backend: 22, frontend: 9, docs: 4, designer: 11, assets: 3 }

export function usageReport(view: TeamView, days: number, now: Date): UsageReport {
  const agents = view.people.filter((person) => person.kind === 'agent')
  const byDay = Array.from({ length: days }, (_, index) => {
    const date = new Date(now.getTime() - (days - 1 - index) * DAY).toISOString().slice(0, 10)
    const costUsd = Object.fromEntries(
      agents.map((person) => [person.id, Math.round((DAILY_BASE_USD[person.id] ?? 5) * (0.35 + 1.3 * noise(`${view.team}:${person.id}:${date}`)) * 100) / 100]),
    )
    return { date, costUsd }
  })
  const byPerson = agents.map((person) => {
    const costUsd = Math.round(byDay.reduce((sum, day) => sum + (day.costUsd[person.id] ?? 0), 0) * 100) / 100
    return {
      person: person.id,
      costUsd,
      tokens: { input: Math.round(costUsd * 9_000), output: Math.round(costUsd * 24_000), cacheRead: Math.round(costUsd * 610_000), cacheWrite: Math.round(costUsd * 38_000) },
    }
  })
  return { org: view.org, team: view.team, days, generatedAt: now.toISOString(), byDay, byPerson: byPerson.toSorted((a, b) => b.costUsd - a.costUsd) }
}
