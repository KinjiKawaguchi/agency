// An in-memory stand-in for a home node's Client API. The same implementation serves the dev server
// (through `vite/mock-node-plugin.ts`) and the component tests (through an injected `fetch`).
import type { DecisionAnswer, DecisionResult, Me, TeamSnapshot, TeamSource } from '../api/clientApi.ts'
import { sampleTeamView, type AnswerWith, type TeamView } from '../api/teamview.ts'
import { fundingSources, ORG_DEFINITION, siteTeamView, usageReport } from './fixtures.ts'

/** The token the mock node's index.html carries and its writes demand. */
export const MOCK_TOKEN = 'mock-token'

export interface MockNode {
  /** Answers a request the way the real node would. */
  handle: (request: Request) => Promise<Response>
  /** Decisions received so far, oldest first. */
  decisions: () => readonly DecisionAnswer[]
}

interface MockNodeOptions {
  /** The moment the sample is shifted to, so times read as "an hour ago" whenever the node starts. */
  now?: Date
  /** Replaces the sample TeamView of the `app` team (already in the node's time frame). */
  view?: TeamView
  /** When set, the node answers every call with this status, to exercise error paths. */
  failWith?: number
  /** Makes the `site` team a stale copy that could not be refreshed for this reason. */
  siteError?: string
  /** Serves token use. Off by default, as the real node does not serve it yet and answers 404. */
  usage?: boolean
  /** Makes only the token use calls fail with this status. */
  usageFailsWith?: number
  /** What the node reports for an accepted decision. */
  outcome?: DecisionResult['outcome']
}

const TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/

/** Shifts every RFC 3339 timestamp by `delta` ms. Calendar dates (`YYYY-MM-DD`) are left alone. */
function shiftTimes<T>(value: T, delta: number): T {
  if (typeof value === 'string') return (TIMESTAMP.test(value) ? new Date(Date.parse(value) + delta).toISOString() : value) as T
  if (Array.isArray(value)) return value.map((item) => shiftTimes(item, delta)) as T
  if (typeof value === 'object' && value !== null) {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, shiftTimes(item, delta)])) as T
  }
  return value
}

const json = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

const failure = (status: number, message: string): Response => json({ error: message }, status)

const ORG_PATH = /^\/api\/orgs\/([^/]+)$/
const TEAM_PATH = /^\/api\/orgs\/([^/]+)\/teams\/([^/]+)\/(view|decisions|usage)$/

const isAnswerWith = (body: unknown): body is AnswerWith & { text: string; choice?: string } =>
  typeof body === 'object' && body !== null && 'kind' in body && 'target' in body && 'text' in body && typeof body.text === 'string'

const teamKey = (org: string, team: string): string => `${org}/${team}`

export function createMockNode(options: MockNodeOptions = {}): MockNode {
  const now = options.now ?? new Date()
  const app: TeamView = options.view ?? shiftTimes(sampleTeamView, now.getTime() - Date.parse(sampleTeamView.generatedAt))
  let views = new Map<string, TeamView>([app, siteTeamView(app)].map((view) => [teamKey(view.org, view.team), view]))
  let received: readonly DecisionAnswer[] = []

  const me: Me = {
    account: { id: 'alice', label: 'Alice', fingerprint: 'a1b2c3d4e5f6' },
    orgs: [
      {
        id: app.org,
        label: ORG_DEFINITION.label,
        isRoot: true,
        teams: [...views.values()].map((view) => ({ id: view.team, label: view.label, location: view.team === 'site' ? 'remote' : 'local', viewer: 'owner' })),
      },
    ],
    funding: fundingSources(now),
  }

  const sourceOf = (view: TeamView): TeamSource =>
    view.team === 'site'
      ? { kind: 'replica', fetchedAt: new Date(now.getTime() - 2 * 60_000).toISOString(), triedAt: new Date(now.getTime() - 30_000).toISOString(), ...(options.siteError === undefined ? {} : { error: options.siteError }) }
      : { kind: 'local' }
  const snapshotOf = (view: TeamView): TeamSnapshot => ({ view, viewer: 'owner', source: sourceOf(view) })

  const decide = async (request: Request, view: TeamView): Promise<Response> => {
    if (request.headers.get('x-agency-token') !== MOCK_TOKEN) return failure(403, 'The request token is missing or wrong.')
    if (request.headers.get('content-type') !== 'application/json') return failure(415, 'Send the decision as application/json.')
    const body: unknown = await request.json().catch(() => null)
    if (!isAnswerWith(body)) return failure(400, 'The decision body is malformed.')
    const target = view.requests.find((one) => one.answerWith.kind === body.kind && one.answerWith.target === body.target)
    if (target === undefined) return failure(404, 'That request is not open.')
    received = [...received, { org: view.org, team: view.team, answerWith: target.answerWith, text: body.text, ...(body.choice === undefined ? {} : { choice: body.choice }) }]
    views = new Map([...views, [teamKey(view.org, view.team), { ...view, requests: view.requests.filter((one) => one !== target) }]])
    return json({ outcome: options.outcome ?? 'applied' } satisfies DecisionResult)
  }

  const handleTeam = async (request: Request, url: URL, [, org = '', team = '', action = '']: RegExpExecArray): Promise<Response> => {
    const view = views.get(teamKey(org, team))
    if (view === undefined) return failure(404, 'No such team.')
    if (request.method === 'GET' && action === 'view') return json(snapshotOf(view))
    if (request.method === 'GET' && action === 'usage') {
      if (options.usageFailsWith !== undefined) return failure(options.usageFailsWith, 'Token usage could not be read.')
      if (options.usage !== true) return failure(404, 'Token usage is not available.')
      const days = Number(url.searchParams.get('days') ?? 14)
      return Number.isInteger(days) && days >= 1 && days <= 90 ? json(usageReport(view, days, now)) : failure(400, 'days must be between 1 and 90.')
    }
    return request.method === 'POST' && action === 'decisions' ? decide(request, view) : failure(404, 'No such route.')
  }

  const handle = async (request: Request): Promise<Response> => {
    if (options.failWith !== undefined) return failure(options.failWith, 'The node is unavailable.')
    const url = new URL(request.url, 'http://mock.local')
    if (request.method === 'GET' && url.pathname === '/api/me') return json(me)
    const orgMatch = ORG_PATH.exec(url.pathname)
    if (request.method === 'GET' && orgMatch) return orgMatch[1] === ORG_DEFINITION.org ? json(ORG_DEFINITION) : failure(404, 'No such organization.')
    const teamMatch = TEAM_PATH.exec(url.pathname)
    return teamMatch ? handleTeam(request, url, teamMatch) : failure(404, 'No such route.')
  }

  return { handle, decisions: () => received }
}
