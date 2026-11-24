// An in-memory stand-in for a home node's Client API. The same implementation serves the dev server
// (through `vite/mock-node-plugin.ts`) and the component tests (through an injected `fetch`).
import type { DecisionAnswer, Me } from '../api/clientApi'
import { sampleTeamView, type AnswerWith, type TeamView } from '../api/teamview'

export interface MockNode {
  /** Answers a request the way the real node would. */
  handle: (request: Request) => Promise<Response>
  /** Decisions received so far, oldest first. */
  decisions: () => readonly DecisionAnswer[]
}

interface MockNodeOptions {
  /** The moment the sample is shifted to, so times read as "an hour ago" whenever the node starts. */
  now?: Date
  /** Replaces the sample TeamView (already in the node's time frame). */
  view?: TeamView
  /** When set, the node answers every call with this status, to exercise error paths. */
  failWith?: number
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

const VIEW_PATH = /^\/api\/orgs\/([^/]+)\/teams\/([^/]+)\/view$/
const DECISION_PATH = /^\/api\/orgs\/([^/]+)\/teams\/([^/]+)\/decisions$/

const isAnswerWith = (body: unknown): body is AnswerWith & { text: string; choice?: string } =>
  typeof body === 'object' && body !== null && 'kind' in body && 'target' in body && 'text' in body && typeof body.text === 'string'

export function createMockNode(options: MockNodeOptions = {}): MockNode {
  const now = options.now ?? new Date()
  const me: Me = {
    account: { id: 'alice', label: 'Alice' },
    orgs: [{ id: sampleTeamView.org, label: 'Acme', teams: [{ id: sampleTeamView.team, label: sampleTeamView.label }] }],
  }
  let view: TeamView = options.view ?? shiftTimes(sampleTeamView, now.getTime() - Date.parse(sampleTeamView.generatedAt))
  let received: readonly DecisionAnswer[] = []

  const decide = async (request: Request): Promise<Response> => {
    const body: unknown = await request.json().catch(() => null)
    if (!isAnswerWith(body)) return failure(400, 'The decision body is malformed.')
    const target = view.requests.find((one) => one.answerWith.kind === body.kind && one.answerWith.target === body.target)
    if (target === undefined) return failure(404, 'That request is not open.')
    received = [...received, { org: view.org, team: view.team, answerWith: target.answerWith, text: body.text, ...(body.choice === undefined ? {} : { choice: body.choice }) }]
    view = { ...view, requests: view.requests.filter((one) => one !== target) }
    return json({})
  }

  const handle = async (request: Request): Promise<Response> => {
    if (options.failWith !== undefined) return failure(options.failWith, 'The node is unavailable.')
    const { pathname } = new URL(request.url, 'http://mock.local')
    if (request.method === 'GET' && pathname === '/api/me') return json(me)
    const viewMatch = VIEW_PATH.exec(pathname)
    if (request.method === 'GET' && viewMatch) {
      return viewMatch[1] === view.org && viewMatch[2] === view.team ? json(view) : failure(404, 'No such team.')
    }
    const decisionMatch = DECISION_PATH.exec(pathname)
    if (request.method === 'POST' && decisionMatch) {
      return decisionMatch[1] === view.org && decisionMatch[2] === view.team ? decide(request) : failure(404, 'No such team.')
    }
    return failure(404, 'No such route.')
  }

  return { handle, decisions: () => received }
}
