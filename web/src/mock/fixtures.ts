// Mock org structure, funding, and token use for the mock node. Every name here is fictional.
// The shapes are the ones the real node will serve; the numbers are made up.
import type { DecisionEntry, Funding, OrgDefinition, TokenUsage } from '../api/clientApi.ts'
import type { TeamView } from '../api/teamview.ts'

const MINUTE = 60_000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/** A made-up Ed25519 public key: 32 bytes in base64, as the type documents. Not a key of anyone. */
const fakeKey = (character: string): string => `${character.repeat(43)}=`

export const ORG_DEFINITION: OrgDefinition = {
  org: 'acme',
  label: 'Acme',
  version: 3,
  prev: '0'.repeat(64),
  root: { account: 'alice', publicKey: fakeKey('A') },
  departments: [
    { id: 'engineering', label: 'Engineering', parent: null, head: 'owner' },
    { id: 'platform', label: 'Platform', parent: 'engineering', head: 'bob' },
    { id: 'design', label: 'Design', parent: null, head: 'carol' },
  ],
  people: [
    { id: 'owner', label: 'Alice', account: 'alice', publicKey: fakeKey('A'), department: 'engineering', role: 'member' },
    { id: 'bob', label: 'Bob', account: 'bob', publicKey: fakeKey('B'), department: 'platform', role: 'member' },
    { id: 'carol', label: 'Carol', account: 'carol', publicKey: fakeKey('C'), department: 'design', role: 'answerer' },
  ],
  teams: [
    { id: 'app', label: 'App', department: 'engineering', node: 'https://alice.example' },
    { id: 'site', label: 'Site', department: 'design', node: 'https://carol.example' },
  ],
  revoked: [],
  at: '2026-11-24T12:00:00Z',
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

export function fundingSources(now: Date): Funding[] {
  const at = (ms: number) => new Date(now.getTime() + ms).toISOString()
  const readAt = at(-3 * MINUTE)
  return [
    {
      id: 'personal',
      label: 'Personal plan',
      kind: 'subscription',
      // Four of seven days in at 52%: on pace. Three of five hours in at 74%: it will run out.
      windows: [
        { name: 'sevenDay', lengthHours: 168, utilization: 52, resetsAt: at(3 * DAY) },
        { name: 'fiveHour', lengthHours: 5, utilization: 74, resetsAt: at(2 * HOUR) },
      ],
      readAt,
    },
    {
      id: 'lab',
      label: 'Lab plan',
      kind: 'subscription',
      // Two of seven days in at 21%: it will not be used up. No five-hour window is running.
      windows: [{ name: 'sevenDay', lengthHours: 168, utilization: 21, resetsAt: at(5 * DAY) }],
      readAt,
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

export function usageReport(view: TeamView, days: number, now: Date): TokenUsage {
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

/** Decisions made before the mock node started, one in each state, newest first. */
export function earlierDecisions(now: Date): DecisionEntry[] {
  const ago = (ms: number): string => new Date(now.getTime() - ms).toISOString()
  const ahead = (ms: number): string => new Date(now.getTime() + ms).toISOString()
  return [
    {
      id: 'e-20261125-0910-0001',
      org: 'acme',
      team: 'site',
      kind: 'answer',
      target: 'r-20261125-090000-f1f1',
      title: 'Which hosting plan should the landing page use?',
      text: 'The free plan is enough until launch.',
      choice: 'free',
      choiceLabel: 'Free plan',
      decidedAt: ago(20 * MINUTE),
      state: { kind: 'queued', attempts: 2, nextAt: ahead(6 * MINUTE), lastError: 'The team’s node did not answer.' },
    },
    {
      id: 'e-20261124-1700-0002',
      org: 'acme',
      team: 'app',
      kind: 'judge',
      target: 'app#12',
      title: 'Rename the export endpoint',
      text: 'Keep the old name for one more release.',
      decidedAt: ago(19 * HOUR),
      state: { kind: 'refused', message: 'That request was already decided.' },
    },
    {
      id: 'e-20261124-1030-0003',
      org: 'acme',
      team: 'app',
      kind: 'judge',
      target: 'app#9',
      title: 'Adopt the new lint rules',
      text: 'Approved.',
      choice: 'approve',
      decidedAt: ago(26 * HOUR),
      state: { kind: 'applied' },
    },
    {
      id: 'e-20261123-1500-0004',
      org: 'acme',
      team: 'site',
      kind: 'answer',
      target: 'r-20261123-140000-a9a9',
      title: 'Use the dark logo on the home page?',
      text: 'Yes, on dark backgrounds only.',
      decidedAt: ago(2 * DAY),
      state: { kind: 'delivered' },
    },
  ]
}
