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

/** Work items with long refs and long titles, in English and in Japanese, all made up. */
const MANY_ITEMS: TeamView['work'] = Array.from({ length: 14 }, (_, index) => ({
  ref: `site-checkout#${1200 + index * 7}`,
  title: index % 2 === 0 ? `Check that the order total stays right when a discount, a gift card, and a shipping change are applied together (case ${index + 1})` : `割引と贈り物のカードと配送先の変更が同時に入っても、注文の合計が正しいままであることを確かめる (その${index + 1})`,
  status: 'inProgress' as const,
  assignee: 'builder',
  points: 2,
}))

/** A second, quiet team, so the org chart and the people screen show more than one. */
export function siteTeamView(base: TeamView): TeamView {
  return {
    ...base,
    team: 'site',
    label: 'Site',
    people: [
      { id: 'designer', label: 'Designer', kind: 'agent', role: 'lead', state: 'working', since: base.generatedAt, status: 'Drafting the pricing page', holding: ['site#3'] },
      // Holds more work than fits on a line, as a person on a busy team does.
      { id: 'builder', label: 'Builder', kind: 'agent', role: 'member', state: 'working', since: base.generatedAt, status: 'Building the checkout flow', holding: MANY_ITEMS.map((item) => item.ref) },
      { id: 'assets', label: 'Assets', kind: 'agent', role: 'member', state: 'idle', status: 'Idle; nothing in Next', holding: [] },
      { id: 'owner', label: 'Alice', kind: 'human', role: 'member', state: 'unknown', status: 'Nothing is waiting for an answer', holding: [] },
    ],
    requests: [],
    work: [{ ref: 'site#3', title: 'Redesign the pricing page', status: 'inProgress', assignee: 'designer', points: 3 }, ...MANY_ITEMS],
    // An epic that was finished two days ago: the team's view keeps it for thirty days, with nothing left in it and no forecast.
    epics: [
      {
        ref: 'site#2',
        title: 'Launch the landing page',
        total: 4,
        done: 4,
        doing: 0,
        remainingPoints: 0,
        blocked: [],
        basis: 'Every child is done.',
        doneAt: new Date(new Date(base.generatedAt).getTime() - 2 * DAY).toISOString(),
      } as TeamView['epics'][number],
    ],
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

/** More orgs for the overview: a team that is stuck, one that is a stale copy, a busy one, and one that cannot be read. */
export const MORE_ORGS = [
  {
    id: 'lab',
    label: 'Lab',
    teams: [
      { id: 'research', label: 'Research', location: 'local' },
      { id: 'notebook', label: 'Notebook', location: 'remote' },
    ],
  },
  {
    id: 'club',
    label: 'Club',
    teams: [
      { id: 'garden', label: 'Garden', location: 'local' },
      { id: 'shop', label: 'Shop', location: 'remote' },
    ],
  },
] as const

/** The team of `MORE_ORGS` that nobody can read: its node does not answer. */
export const UNREADABLE_TEAM = 'club/shop'

/** The team of `MORE_ORGS` whose data is a day old, because its node could not be reached. */
export const STALE_TEAM = 'lab/notebook'

const agent = (id: string, label: string, role: 'lead' | 'member', state: 'working' | 'idle', status: string, since: string, holding: string[] = []): TeamView['people'][number] => ({
  id,
  label,
  kind: 'agent',
  role,
  state,
  since,
  status,
  holding,
})

const owner: TeamView['people'][number] = { id: 'owner', label: 'Alice', kind: 'human', role: 'member', state: 'unknown', status: 'Nothing is waiting for an answer', holding: [] }

/** The views of the teams in `MORE_ORGS` that can be read, in the node's time frame. */
export function moreTeamViews(now: Date): TeamView[] {
  const ago = (ms: number): string => new Date(now.getTime() - ms).toISOString()
  const day = (days: number): string => new Date(now.getTime() - days * DAY).toISOString().slice(0, 10)
  const base = { epics: [] }
  const research: TeamView = {
    ...base,
    org: 'lab',
    team: 'research',
    label: 'Research',
    generatedAt: ago(MINUTE),
    people: [
      agent('quinn', 'Quinn', 'lead', 'idle', 'Idle: three items wait on a decision', ago(3 * HOUR)),
      agent('rae', 'Rae', 'member', 'idle', 'Waiting for the data export to finish', ago(5 * HOUR), ['lab#4']),
      owner,
    ],
    requests: [
      {
        id: 'r-20261124-090000-e5f6',
        kind: 'question',
        title: 'Which sample should the next run use?',
        context: 'Both samples are ready.',
        from: 'quinn',
        to: 'bob',
        createdAt: ago(3 * HOUR),
        blocks: ['quinn'],
        options: [],
        answerWith: { kind: 'answer', target: 'r-20261124-090000-e5f6' },
      },
    ],
    work: [
      { ref: 'lab#2', title: 'Choose the sampling rate', status: 'next', blocked: { reason: 'needsDecision', since: day(3) }, points: 2 },
      { ref: 'lab#3', title: 'Pick the control group', status: 'next', blocked: { reason: 'needsDecision', since: day(2) }, points: 3 },
      { ref: 'lab#4', title: 'Collect the second batch', status: 'inProgress', assignee: 'rae', blocked: { reason: 'external', since: day(1) }, points: 5 },
      { ref: 'lab#5', title: 'Write up the first batch', status: 'done', assignee: 'quinn', points: 2 },
    ],
  }
  const notebook: TeamView = {
    ...base,
    org: 'lab',
    team: 'notebook',
    label: 'Notebook',
    generatedAt: ago(26 * HOUR),
    people: [
      agent('nova', 'Nova', 'lead', 'working', 'Reviewing the draft', ago(26 * HOUR), ['nb#7']),
      agent('kit', 'Kit', 'member', 'working', 'Cleaning the figures', ago(27 * HOUR), ['nb#8']),
      owner,
    ],
    requests: [
      {
        id: 'r-20261123-100000-a7b8',
        kind: 'pendingDecision',
        title: 'Publish the notebook under the lab’s name?',
        context: 'The draft is ready for review.',
        from: 'nb#7',
        createdAt: ago(27 * HOUR),
        blocks: [],
        options: [],
        answerWith: { kind: 'judge', target: 'nb#7' },
      },
    ],
    work: [
      { ref: 'nb#7', title: 'Review the draft', status: 'inProgress', assignee: 'nova', points: 2 },
      { ref: 'nb#8', title: 'Clean the figures', status: 'inProgress', assignee: 'kit', points: 1 },
    ],
  }
  const garden: TeamView = {
    ...base,
    org: 'club',
    team: 'garden',
    label: 'Garden',
    generatedAt: ago(MINUTE),
    people: [
      agent('ivy', 'Ivy', 'lead', 'working', 'Planning the spring planting', ago(HOUR), ['gd#1']),
      agent('moss', 'Moss', 'member', 'working', 'Updating the seed list', ago(2 * HOUR), ['gd#2']),
      agent('fern', 'Fern', 'member', 'idle', 'Nothing in Next', ago(6 * HOUR)),
      owner,
    ],
    requests: [],
    work: [
      { ref: 'gd#1', title: 'Plan the spring planting', status: 'inProgress', assignee: 'ivy', points: 3 },
      { ref: 'gd#2', title: 'Update the seed list', status: 'inProgress', assignee: 'moss', points: 1 },
    ],
  }
  return [research, notebook, garden]
}

/** A made-up definition for an org of `MORE_ORGS`: the account is its root, and every team runs on its own node. */
export function moreOrgDefinition(org: (typeof MORE_ORGS)[number]): OrgDefinition {
  return {
    org: org.id,
    label: org.label,
    version: 1,
    prev: '0'.repeat(64),
    root: { account: 'alice', publicKey: fakeKey('A') },
    departments: [{ id: 'main', label: 'Main', parent: null, head: 'owner' }],
    people: [{ id: 'owner', label: 'Alice', account: 'alice', publicKey: fakeKey('A'), department: 'main', role: 'member' }],
    teams: org.teams.map((team) => ({ id: team.id, label: team.label, department: 'main', node: 'https://alice.example' })),
    revoked: [],
    at: '2026-11-24T12:00:00Z',
  }
}
