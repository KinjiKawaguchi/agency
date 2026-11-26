// An in-memory stand-in for a home node's Client API. The same implementation serves the dev server
// (through `vite/mock-node-plugin.ts`) and the component tests (through an injected `fetch`).
import type { DecisionAnswer, DecisionEntry, Device, Envelope, Signed, DecisionLog, DecisionResult, Me, Session, TeamSnapshot, SnapshotSource } from '../api/clientApi.ts'
import { sampleTeamView, type AnswerWith, type TeamView } from '../api/teamview.ts'
import { createMockDevices, isRefusal } from './devices.ts'
import { activityFor, MAX_EVENTS } from './activity.ts'
import { effortReport, withPlans } from './effort.ts'
import { evaluationReport } from './evaluation.ts'
import { earlierDecisions, fundingSources, MORE_ORGS, moreOrgDefinition, moreTeamViews, ORG_DEFINITION, siteTeamView, STALE_TEAM, UNREADABLE_TEAM, usageReport } from './fixtures.ts'

/** What the mock node says approves a first device on its machine. */
export const MOCK_DEVICE_COMMAND = '/usr/local/bin/agency --dir /var/agency device-add'

/** The token the mock node's index.html carries and its writes demand. */
export const MOCK_TOKEN = 'mock-token'

export interface MockNode {
  /** Answers a request the way the real node would. */
  handle: (request: Request) => Promise<Response>
  /** What `agency device-add` does on the node's terminal: adds the device that asked with this pairing code. */
  addDeviceFromTerminal: (pairingCode: string) => boolean
  /** Push tokens registered so far, one entry per token, as the node keeps them. */
  pushTokens: () => ReadonlyArray<{ token: string; environment: string; label: string }>
  /** Envelopes that were accepted, oldest first. */
  envelopes: () => readonly Signed<Envelope>[]
  /** Starts the node again: its write token changes, as it does each time the real node starts, and a page that holds the old one is refused. */
  restart: () => void
  /** Makes the node answer every call as Cloudflare Access does once the session has expired: a redirect to its login on another origin. */
  setSignInExpired: (isExpired: boolean) => void
  /** Decisions received so far, oldest first. They also appear in the decision log. */
  decisions: () => readonly DecisionAnswer[]
}

interface MockNodeOptions {
  /** The moment the sample is shifted to, so times read as "an hour ago" whenever the node starts. */
  now?: Date
  /** Replaces the sample TeamView of the `app` team (already in the node's time frame). */
  view?: TeamView
  /** When set, the node answers every call with this status, to exercise error paths. */
  failWith?: number
  /** Adds the orgs Lab and Club, with a stuck team, a stale copy, a busy team, and one that cannot be read. */
  moreOrgs?: boolean
  /** Makes the `site` team a stale copy that could not be refreshed for this reason. */
  siteError?: string
  /** Serves token use. Off by default, as the real node does not serve it yet and answers 404. */
  usage?: boolean
  /** Makes only the token use calls fail with this status. */
  usageFailsWith?: number
  /** Serves the evaluation numbers. Off by default, as the real node does not serve them yet and answers 404. */
  evaluation?: boolean
  /** Makes only the evaluation calls fail with this status. */
  evaluationFailsWith?: number
  /** A team the evaluation routes leave out, naming it in `unavailable` with a reason, as the node does for a team that cannot answer. */
  evaluationUnavailableTeam?: string
  /** Serves the activity feed (the live screen). Off by default, as the real node does not serve it yet and answers 404. */
  activity?: boolean
  /** Makes only the activity calls fail with this status. */
  activityFailsWith?: number
  /** The time the feed is cut at: the clock by default, so that the feed goes on as the screen stays open. */
  activityClock?: () => number
  /** Devices the account already has in its org, so a test can decide without going through adding one. */
  devices?: ReadonlyArray<Pick<Device, 'id' | 'label' | 'publicKey'>>
  /** False to leave out the node's own command for approving a device, as the node does for anyone but its owner. */
  deviceCommand?: boolean
  /** Starts with no request waiting on anyone, as a desk that has been cleared. */
  nothingWaiting?: boolean
  /** Makes `POST /api/push/tokens` answer this status, as a node that does not serve it yet (404) or is failing. */
  pushTokensFailWith?: number
  /** Makes only the decision log calls fail with this status. */
  decisionLogFailsWith?: number
  /** Makes the team's node refuse every decision for this reason (HTTP 409). */
  refuseDecisionsWith?: string
  /** Starts the decision log with a few made earlier, one in each state. On by default; `false` starts it empty. */
  earlierDecisions?: boolean
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

const MINUTE = 60_000

/** Where an expired session is sent: another origin, as the access login is. */
const SIGN_IN_URL = 'https://login.agency.test/cdn-cgi/access/login'
const DECISIONS_PATH = '/api/decisions'
const DEFAULT_LIMIT = 50
const MAX_LIMIT = 200

/** The longest `title` and `choiceLabel` the node takes. */
const MAX_NOTE = 200
const ORG_PATH = /^\/api\/orgs\/([^/]+)$/
const EVALUATION_PATH = /^\/api\/orgs\/([^/]+)\/(evaluation|effort)$/
const TEAM_PATH = /^\/api\/orgs\/([^/]+)\/teams\/([^/]+)\/(view|usage|activity|envelopes)$/
const DEVICES_PATH = '/api/devices'
const PUSH_TOKENS_PATH = '/api/push/tokens'

const teamKey = (org: string, team: string): string => `${org}/${team}`

export function createMockNode(options: MockNodeOptions = {}): MockNode {
  const now = options.now ?? new Date()
  const sample: TeamView = options.view ?? shiftTimes(sampleTeamView, now.getTime() - Date.parse(sampleTeamView.generatedAt))
  const app: TeamView = options.nothingWaiting === true ? { ...sample, requests: [], people: sample.people.map(({ waitingOn: _waitingOn, ...rest }) => rest) } : sample
  const more = options.moreOrgs === true ? moreTeamViews(now) : []
  let views = new Map<string, TeamView>([app, siteTeamView(app), ...more].map((view) => [teamKey(view.org, view.team), view]))
  let received: readonly DecisionAnswer[] = []
  let isSignInExpired = false
  let writeToken = MOCK_TOKEN
  let accepted: readonly Signed<Envelope>[] = []
  let pushTokens: ReadonlyArray<{ token: string; environment: string; label: string }> = []
  let log: readonly DecisionEntry[] = options.earlierDecisions === false ? [] : earlierDecisions(now)

  const me: Me = {
    account: { id: 'alice', label: 'Alice', fingerprint: 'a1b2c3d4e5f6' },
    orgs: [
      {
        id: app.org,
        label: ORG_DEFINITION.label,
        isRoot: true,
        teams: [app, siteTeamView(app)].map((view) => ({ id: view.team, label: view.label, location: view.team === 'site' ? 'remote' : 'local', viewer: 'owner' })),
      },
      ...(options.moreOrgs === true
        ? MORE_ORGS.map((org) => ({ id: org.id, label: org.label, isRoot: true, teams: org.teams.map((team) => ({ id: team.id, label: team.label, location: team.location, viewer: 'owner' })) }))
        : []),
    ],
    funding: fundingSources(now),
  }

  const sourceOf = (view: TeamView): SnapshotSource =>
    teamKey(view.org, view.team) === STALE_TEAM
      ? { kind: 'replica', fetchedAt: view.generatedAt, triedAt: new Date(now.getTime() - 5 * MINUTE).toISOString(), error: 'The team’s node did not answer.' }
      : view.team === 'site'
      ? { kind: 'replica', fetchedAt: new Date(now.getTime() - 2 * 60_000).toISOString(), triedAt: new Date(now.getTime() - 30_000).toISOString(), ...(options.siteError === undefined ? {} : { error: options.siteError }) }
      : { kind: 'local' }
  // The node's rule for the one viewer here, the team's owner: a request with no addressee, or addressed to them.
  const mineOf = (view: TeamView): string[] => view.requests.filter((request) => (request.to ?? 'owner') === 'owner').map((request) => request.id)
  // And who waits on them: the people whose `waitingOn` names the viewer.
  const waitingOnViewerOf = (view: TeamView): string[] => view.people.filter((person) => person.waitingOn?.kind === 'person' && person.waitingOn.person === 'owner').map((person) => person.id)
  const snapshotOf = (view: TeamView): TeamSnapshot => ({ view, viewer: 'owner', mine: mineOf(view), waitingOnViewer: waitingOnViewerOf(view), source: sourceOf(view) })

  /** A write must carry the page's token and say it is JSON; answers the refusal, or the parsed body. */
  const readWrite = async (request: Request): Promise<Response | { body: unknown }> => {
    if (request.headers.get('x-agency-token') !== writeToken) return failure(403, 'missing or wrong write token')
    if (request.headers.get('content-type') !== 'application/json') return failure(415, 'Send the body as application/json.')
    return { body: await request.json().catch(() => null) }
  }

  /** The node's own account, with the devices it holds: the signed routes check against them. */
  const devices = createMockDevices([ORG_DEFINITION.org], me.account.id, options.devices ?? [])

  const decideSigned = async (request: Request, view: TeamView): Promise<Response> => {
    const write = await readWrite(request)
    if (write instanceof Response) return write
    const { body } = write
    if (typeof body !== 'object' || body === null || !('envelope' in body)) return failure(400, 'The decision body is malformed.')
    const refusal = await devices.checkEnvelope(view.org, view.team, body.envelope)
    if (refusal !== undefined) return failure(refusal.status, refusal.message)
    if (options.refuseDecisionsWith !== undefined) return failure(409, options.refuseDecisionsWith)
    const signed = body.envelope as Signed<Envelope>
    const { kind, target, payload } = signed.body
    const notes = body as { title?: string; choiceLabel?: string }
    // The node's note limit, counted in characters (Rust `chars`), not UTF-16 units.
    for (const [field, text] of [['title', notes.title], ['choiceLabel', notes.choiceLabel]] as const) {
      if (text !== undefined && [...text.trim()].length > MAX_NOTE) return failure(400, `${field} is at most ${MAX_NOTE} characters`)
    }
    const reply = await accept(view, { id: signed.body.id, kind, target, text: payload.text, ...(payload.choice === undefined ? {} : { choice: payload.choice }), ...(notes.title === undefined ? {} : { title: notes.title }), ...(notes.choiceLabel === undefined ? {} : { choiceLabel: notes.choiceLabel }) })
    if (reply.ok) accepted = [...accepted, signed]
    return reply
  }

  const accept = async (view: TeamView, body: AnswerWith & { id?: string; text: string; choice?: string; choiceLabel?: string; title?: string }): Promise<Response> => {
    const target = view.requests.find((one) => one.answerWith.kind === body.kind && one.answerWith.target === body.target)
    if (target === undefined) return failure(404, 'That request is not open.')
    received = [...received, { org: view.org, team: view.team, answerWith: target.answerWith, title: body.title ?? '', text: body.text, ...(body.choice === undefined ? {} : { choice: body.choice }), ...(body.choiceLabel === undefined ? {} : { choiceLabel: body.choiceLabel }) }]
    views = new Map([...views, [teamKey(view.org, view.team), { ...view, requests: view.requests.filter((one) => one !== target) }]])
    const outcome = options.outcome ?? 'applied'
    const state: DecisionEntry['state'] =
      outcome === 'queued' ? { kind: 'queued', attempts: 0, nextAt: new Date(now.getTime() + 5 * MINUTE).toISOString() } : { kind: outcome === 'applied' ? 'applied' : 'delivered' }
    log = [
      // The log names a decision by its envelope's id, as the real node does.
      { id: body.id ?? `e-${log.length + 1}`, org: view.org, team: view.team, kind: body.kind, target: body.target, text: body.text, decidedAt: new Date().toISOString(), state, ...(body.title === undefined ? {} : { title: body.title }), ...(body.choice === undefined ? {} : { choice: body.choice }), ...(body.choiceLabel === undefined ? {} : { choiceLabel: body.choiceLabel }) },
      ...log,
    ]
    return json({ outcome, ...(outcome === 'queued' ? { message: 'Its node is not reachable right now.' } : {}) } satisfies DecisionResult)
  }

  const handleTeam = async (request: Request, url: URL, [, org = '', team = '', action = '']: RegExpExecArray): Promise<Response> => {
    if (teamKey(org, team) === UNREADABLE_TEAM && options.moreOrgs === true) return failure(503, 'The team’s node has not been reached yet.')
    const view = views.get(teamKey(org, team))
    if (view === undefined) return failure(404, 'No such team.')
    if (request.method === 'GET' && action === 'view') return json(snapshotOf(view))
    if (request.method === 'GET' && action === 'usage') {
      if (options.usageFailsWith !== undefined) return failure(options.usageFailsWith, 'Token usage could not be read.')
      if (options.usage !== true) return failure(404, 'Token usage is not available.')
      const days = Number(url.searchParams.get('days') ?? 14)
      return Number.isInteger(days) && days >= 1 && days <= 90 ? json(usageReport(view, days, now)) : failure(400, 'days must be between 1 and 90.')
    }
    if (request.method === 'GET' && action === 'activity') {
      if (options.activityFailsWith !== undefined) return failure(options.activityFailsWith, 'The activity could not be read.')
      if (options.activity !== true) return failure(404, 'The activity is not available.')
      const since = Date.parse(url.searchParams.get('since') ?? '')
      if (Number.isNaN(since)) return failure(400, 'since must be a time.')
      const limit = Math.min(MAX_EVENTS, Math.max(1, Number(url.searchParams.get('limit') ?? MAX_EVENTS)))
      return json(activityFor(view, since, (options.activityClock ?? Date.now)(), limit))
    }
    return request.method === 'POST' && action === 'envelopes' ? decideSigned(request, view) : failure(404, 'No such route.')
  }

  const handleEvaluation = (url: URL, org: string, route: string): Response => {
    if (options.evaluationFailsWith !== undefined) return failure(options.evaluationFailsWith, 'The evaluation could not be read.')
    if (options.evaluation !== true) return failure(404, 'The evaluation is not available.')
    const known = me.orgs.find((one) => one.id === org)
    if (known === undefined) return failure(404, 'No such organization.')
    const days = Number(url.searchParams.get('days') ?? 14)
    if (!Number.isInteger(days) || days < 1 || days > 90) return failure(400, 'days must be between 1 and 90.')
    const all = known.teams.map((team) => team.id)
    const left = options.evaluationUnavailableTeam
    const teams = all.filter((team) => team !== left)
    const report = route === 'effort' ? effortReport(org, teams, days, now) : evaluationReport(org, teams, days, now)
    return json(left !== undefined && all.includes(left) ? { ...report, unavailable: [{ team: left, reason: 'Its node did not answer.' }] } : report)
  }

  /** The owner's write: the page's token and JSON, a lowercase-hex token, its environment, and a label. Idempotent per token. */
  const registerPushToken = async (request: Request): Promise<Response> => {
    if (options.pushTokensFailWith !== undefined) return failure(options.pushTokensFailWith, 'The node does not take push tokens.')
    const write = await readWrite(request)
    if (write instanceof Response) return write
    const { token, environment, label } = (write.body ?? {}) as Record<string, unknown>
    if (typeof token !== 'string' || !/^[0-9a-f]{32,200}$/.test(token)) return failure(400, 'The token is hex of 32 to 200 characters.')
    if (environment !== 'sandbox' && environment !== 'production') return failure(400, 'The environment is sandbox or production.')
    if (typeof label !== 'string' || label.trim() === '') return failure(400, 'The label is missing.')
    pushTokens = [...pushTokens.filter((one) => one.token !== token), { token, environment, label }]
    return json({})
  }

  const handleDevices = async (request: Request, url: URL): Promise<Response> => {
    // The node tells its owner the command that approves a device on its machine; a loopback request is the owner's.
    if (request.method === 'GET' && url.pathname === DEVICES_PATH) return json(options.deviceCommand === false ? devices.list() : { ...devices.list(), node: { url: 'http://127.0.0.1:4400', dataDir: '/var/agency', command: MOCK_DEVICE_COMMAND } })
    const write = request.method === 'POST' ? await readWrite(request) : undefined
    if (write === undefined) return failure(404, 'No such route.')
    if (write instanceof Response) return write
    if (url.pathname === `${DEVICES_PATH}/requests`) {
      const asked = await devices.request(write.body)
      return isRefusal(asked) ? failure(asked.status, asked.message) : json(asked)
    }
    if (url.pathname === `${DEVICES_PATH}/changes`) {
      const refusal = await devices.change(write.body)
      return refusal === undefined ? json({ outcome: 'applied' }) : failure(refusal.status, refusal.message)
    }
    return failure(404, 'No such route.')
  }

  const handle = async (request: Request): Promise<Response> => {
    if (isSignInExpired) return new Response(null, { status: 302, headers: { location: SIGN_IN_URL } })
    if (options.failWith !== undefined) return failure(options.failWith, 'The node is unavailable.')
    const url = new URL(request.url, 'http://mock.local')
    if (request.method === 'GET' && url.pathname === '/api/me') return json(me)
    if (request.method === 'GET' && url.pathname === '/api/session') return json({ writeToken, now: new Date().toISOString() } satisfies Session)
    if (request.method === 'GET' && url.pathname === DECISIONS_PATH) {
      if (options.decisionLogFailsWith !== undefined) return failure(options.decisionLogFailsWith, 'The decision log could not be read.')
      const limit = Number(url.searchParams.get('limit') ?? DEFAULT_LIMIT)
      return json({ entries: log.slice(0, Math.min(MAX_LIMIT, Math.max(1, limit))) } satisfies DecisionLog)
    }
    if (url.pathname === PUSH_TOKENS_PATH && request.method === 'POST') return registerPushToken(request)
    if (url.pathname.startsWith(DEVICES_PATH)) return handleDevices(request, url)
    const orgMatch = ORG_PATH.exec(url.pathname)
    if (request.method === 'GET' && orgMatch) {
      const extra = options.moreOrgs === true ? MORE_ORGS.find((org) => org.id === orgMatch[1]) : undefined
      if (extra !== undefined) return json(moreOrgDefinition(extra))
      return orgMatch[1] === ORG_DEFINITION.org ? json({ ...ORG_DEFINITION, teams: withPlans(ORG_DEFINITION.teams) }) : failure(404, 'No such organization.')
    }
    const evaluationMatch = EVALUATION_PATH.exec(url.pathname)
    if (request.method === 'GET' && evaluationMatch) return handleEvaluation(url, evaluationMatch[1] ?? '', evaluationMatch[2] ?? '')
    const teamMatch = TEAM_PATH.exec(url.pathname)
    return teamMatch ? handleTeam(request, url, teamMatch) : failure(404, 'No such route.')
  }

  return {
    handle,
    restart: () => {
      writeToken = `${MOCK_TOKEN}-restarted`
    },
    setSignInExpired: (isExpired) => {
      isSignInExpired = isExpired
    },
    decisions: () => received,
    envelopes: () => accepted,
    pushTokens: () => pushTokens,
    addDeviceFromTerminal: devices.addFromTerminal,
  }
}
