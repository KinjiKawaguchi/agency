import type { ClaimInput, ClaimState } from '../claim/provisional'
import { labelsMadePrintable } from '../shared/printable'
import { looksLikeExpiredSignIn, SignInExpiredError } from './session'
import type { DecisionLog, DecisionResult, DeviceChange, DeviceChangeResult, DeviceList, DeviceRequest, DeviceRequestInput, Me, PushSubscription, PushSubscriptionInput, PushSubscriptionList, PushTokenInput, Session, VapidKey, Signed, SignedDecisionInput, OrgDefinition, TeamSnapshot, TokenUsage } from './clientApi'
import { MalformedAnswer, readEffort, readEvaluation } from './evaluationAnswers'
import { readActivity } from './activityAnswers'
import type { Activity } from './activityApi'
import type { Effort, Evaluation } from './evaluationApi'
import type { Metrics } from './metricsApi'
import { readMetrics } from './metricsAnswers'
import type { NumberEntry, NumberInput, Outcomes, Readings } from './outcomesApi'
import { readNumberEntry, readNumberHistory, readOutcomes, readReadings } from './outcomesAnswers'

export type FetchLike = (input: Request) => Promise<Response>

/**
 * An error from the node, carrying the status so callers can tell them apart: 404 is gone (or not served),
 * 409 is a refusal by the team's node, and a runtime's own 400 and 404 pass through unchanged.
 */
export class ApiError extends Error {
  readonly status: number
  /** How many seconds the node asked to wait before trying again (`Retry-After`), when it did. */
  readonly retryAfterSeconds: number | undefined

  constructor(message: string, status: number, retryAfterSeconds?: number) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.retryAfterSeconds = retryAfterSeconds
  }
}

/** What the client needs to know about the page it runs in. */
export interface PageEnvironment {
  hostname: () => string
  isOnline: () => boolean
}

const browserPage: PageEnvironment = { hostname: () => window.location.hostname, isOnline: () => navigator.onLine }

export type SessionState = 'ok' | 'expired'

export interface ApiClient {
  /** Tells when calls start failing because the sign-in expired, and when one works again. Returns the way to stop. */
  onSession: (listener: (state: SessionState) => void) => () => void
  me: () => Promise<Me>
  teamSnapshot: (org: string, team: string) => Promise<TeamSnapshot>
  orgDefinition: (org: string) => Promise<OrgDefinition>
  /** The account's recent decisions, newest first. */
  decisions: (limit: number) => Promise<DecisionLog>
  /** Sends a decision already signed by one of this account's devices. */
  decideSigned: (org: string, team: string, input: SignedDecisionInput) => Promise<DecisionResult>
  /** The account's devices in each org, and the devices waiting to be added. */
  devices: () => Promise<DeviceList>
  /** Registers this phone's push token with the node. Idempotent: the node keeps one entry per token. */
  registerPushToken: (input: PushTokenInput) => Promise<void>
  /** The node's Web Push key, which a browser subscribes with. 404 (an `ApiError`) when the node has no Web Push. */
  vapidKey: () => Promise<VapidKey>
  /** The browsers subscribed to the owner's notifications. */
  pushSubscriptions: () => Promise<PushSubscriptionList>
  /** Registers this browser's subscription. Idempotent: the node keeps one entry per endpoint, and answers it with its `id`. */
  registerPushSubscription: (input: PushSubscriptionInput) => Promise<PushSubscription>
  /** Removes a subscription by its `id`. Removing one that is gone is not an error. */
  removePushSubscription: (id: string) => Promise<void>
  /** Asks the node to add this device; a device that is already added confirms it after comparing the pairing codes. */
  requestDevice: (input: DeviceRequestInput) => Promise<DeviceRequest>
  /** Adds a device: a change signed by one of the account's devices. */
  submitDeviceChange: (change: Signed<DeviceChange>) => Promise<DeviceChangeResult>
  /** Takes a seat with the code of a claim link and the key this browser made; the person's sign-in is the other half of the proof. */
  redeemClaim: (input: ClaimInput) => Promise<ClaimState>
  /** Where this sign-in's claim stands, for a page that is read again or waits for the owner: 404 when there is none. */
  claimState: () => Promise<ClaimState>
  usage: (org: string, team: string, days: number) => Promise<TokenUsage>
  /** How fast each team of the org ships: merged pull requests, lead times, red main, reverts, cost. */
  evaluation: (org: string, days: number) => Promise<Evaluation>
  /** What each team of the org spent, by work type. */
  effort: (org: string, days: number) => Promise<Effort>
  /** The numbers of the org's KPIs over the last `days` days: owner only (403 for anyone else, which is "nothing to show", not a failure). */
  outcomes: (org: string, days: number) => Promise<Outcomes>
  /** How each aim's KPI stands around the day its epic shipped: owner only, as `outcomes`. */
  readings: (org: string) => Promise<Readings>
  /** A KPI's history, newest first, the entries a later one superseded included: owner only. */
  numberHistory: (org: string, kpi: string, limit: number) => Promise<NumberEntry[]>
  /** Enters a number of a KPI for a day (the node sets who and when); a second one for the same day wins and the first stays in the history. Owner only. */
  enterNumber: (org: string, input: NumberInput) => Promise<NumberEntry>
  /** What the node measured of its own response times over the last `days` days (1 to 30): owner only (403 for anyone else, which is "nothing to show"), and 404 from a node that does not measure. */
  metrics: (days: number, hourBuckets?: boolean) => Promise<Metrics>
  /** What happened in a team after `since` (an RFC 3339 time): board events and messages between its agents, oldest first. */
  activity: (org: string, team: string, since: string, signal?: AbortSignal) => Promise<Activity>
}

const segment = encodeURIComponent

/** Reads an answer the node gave; one that is not what the route promises is a 503, "could not answer", not a crash of the screen. */
const checked =
  <T,>(read: (raw: unknown) => T) =>
  (raw: unknown): T => {
    try {
      return read(raw)
    } catch (error) {
      throw error instanceof MalformedAnswer ? new ApiError(error.message, 503) : error
    }
  }

/** The longest title or option label the node keeps in its decision log. */
const MAX_LOGGED_TEXT = 200

/**
 * Trimmed, and when longer than the node keeps, cut so that the text and its ellipsis fit. The node
 * counts Unicode scalar values, as Rust `chars` does, so the count is by code point, not UTF-16 unit.
 */
export function forLog(text: string): string {
  const characters = [...text.trim()]
  if (characters.length <= MAX_LOGGED_TEXT) return characters.join('')
  return `${characters.slice(0, MAX_LOGGED_TEXT - 1).join('').trimEnd()}…`
}

/** What the node says when a write's token is not the one it holds. */
const STALE_TOKEN = 'write token'

/** The node puts a token for this page in its served index.html; every write sends it back. */
function readToken(): string {
  return document.querySelector('meta[name="agency-token"]')?.getAttribute('content') ?? '' /* stand-in-ok(device): the write token the node put in this page, not a value the node answers with */
}

async function readJson<T>(response: Response): Promise<T> {
  const body: unknown = await response.json().catch(() => ({}))
  if (!response.ok) {
    const message = typeof body === 'object' && body !== null && 'error' in body ? String(body.error) : `HTTP ${response.status}`
    const waited = Number(response.headers.get('retry-after'))
    throw new ApiError(message, response.status, Number.isInteger(waited) && waited >= 0 && response.headers.get('retry-after') !== null ? waited : undefined)
  }
  // The labels in an answer are drawn as they are printed on a terminal: a hidden character is a `?`.
  return labelsMadePrintable(body) as T
}

/** How long after a call failed without an answer the node is asked whether the sign-in is really gone. */
const CONFIRM_AFTER_MS = 1000

export interface ClientOptions {
  /** For tests: the wait before the node is asked whether the sign-in is gone. */
  confirmAfterMs?: number
}

/** A redirect that was not followed: what a browser answers for one with `redirect: 'manual'`, or, from a stand-in for the browser, a 3xx. */
const isRedirected = (answer: Response): boolean => answer.type === 'opaqueredirect' || (answer.status >= 300 && answer.status < 400)

/** A call the page cancelled itself (a screen closed, a read replaced by another): it says nothing about the sign-in. */
const wasCancelled = (cause: unknown, request: Request): boolean => request.signal.aborted || (cause instanceof DOMException && cause.name === 'AbortError')

/** Talks to the home node. `send` is injectable so tests can route calls to the mock node. */
export function createApiClient(send: FetchLike = (request) => fetch(request), page: PageEnvironment = browserPage, { confirmAfterMs = CONFIRM_AFTER_MS }: ClientOptions = {}): ApiClient {
  const url = (path: string): URL => new URL(path, window.location.origin)
  const listeners = new Set<(state: SessionState) => void>()
  const tell = (state: SessionState): void => listeners.forEach((listener) => listener(state))

  /**
   * Whether the sign-in is really gone: asked a moment after a call failed without an answer, with one call to
   * the node that takes no part in anything else, and that does not follow a redirect. Access's redirect to its
   * login then comes back as what it is (an `opaqueredirect`), which is the one sign of an expired sign-in; a
   * failure of the connection stays a network error, and any other answer means the sign-in holds. Calls that
   * failed at the same moment share the one asking.
   */
  let confirming: Promise<boolean> | undefined
  const isSignInGone = (): Promise<boolean> => {
    confirming ??= (async () => {
      await new Promise((resolve) => setTimeout(resolve, confirmAfterMs))
      if (!looksLikeExpiredSignIn({ hostname: page.hostname() }, page.isOnline())) return false
      return send(new Request(url('/api/session'), { headers: { accept: 'application/json' }, redirect: 'manual' })).then(isRedirected, () => false)
    })().finally(() => {
      confirming = undefined
    })
    return confirming
  }

  /**
   * Sends a call. One that fails without an answer, away from this machine, may be the redirect Access makes once the
   * sign-in expired, but a lost connection fails the same way, as does a call the page cancelled. It is taken for an
   * expired sign-in only when the node, asked a moment later, answers nothing either; otherwise it fails as it did.
   */
  const dispatch = async (request: Request): Promise<Response> => {
    let response: Response
    try {
      response = await send(request)
    } catch (cause) {
      if (wasCancelled(cause, request) || !looksLikeExpiredSignIn({ hostname: page.hostname() }, page.isOnline()) || !(await isSignInGone())) throw cause
      tell('expired')
      throw new SignInExpiredError()
    }
    tell('ok')
    return response
  }
  const get = <T>(path: string, signal?: AbortSignal): Promise<T> => dispatch(new Request(url(path), { headers: { accept: 'application/json' }, ...(signal === undefined ? {} : { signal }) })).then(readJson<T>)

  /** The page's token goes stale when the node starts again: a token read from `/api/session` after that is kept for the writes that follow. */
  let renewedToken: string | undefined

  const write = <T>(method: 'POST' | 'DELETE', path: string, body: unknown, token: string): Promise<T> =>
    dispatch(new Request(url(path), { method, headers: { ...(body === undefined ? {} : { 'content-type': 'application/json' }), 'x-agency-token': token }, ...(body === undefined ? {} : { body: JSON.stringify(body) }) })).then(readJson<T>)

  /**
   * A write: JSON with the page's token, which the node demands of every write. The token changes each time the node
   * starts, so a page that was open across a start gets 403 "missing or wrong write token": it asks the node for the
   * token once, and sends the write again once (docs/protocol.md, `/api/session`). A second refusal, or any other, is the answer.
   */
  const submit = async <T>(method: 'POST' | 'DELETE', path: string, body: unknown): Promise<T> => {
    try {
      return await write<T>(method, path, body, renewedToken ?? readToken())
    } catch (error) {
      if (!(error instanceof ApiError) || error.status !== 403 || !error.message.includes(STALE_TOKEN)) throw error
      renewedToken = (await get<Session>('/api/session')).writeToken
      return write<T>(method, path, body, renewedToken)
    }
  }
  const post = <T>(path: string, body: unknown): Promise<T> => submit<T>('POST', path, body)
  /** A DELETE has the write token and no body. */
  const remove = (path: string): Promise<unknown> => submit<unknown>('DELETE', path, undefined)

  return {
    onSession: (listener) => {
      listeners.add(listener)
      return () => void listeners.delete(listener)
    },
    me: () => get<Me>('/api/me'),
    teamSnapshot: (org, team) => get<TeamSnapshot>(`/api/orgs/${segment(org)}/teams/${segment(team)}/view`),
    orgDefinition: (org) => get<OrgDefinition>(`/api/orgs/${segment(org)}`),
    usage: (org, team, days) => get<TokenUsage>(`/api/orgs/${segment(org)}/teams/${segment(team)}/usage?days=${days}`),
    evaluation: (org, days) => get<unknown>(`/api/orgs/${segment(org)}/evaluation?days=${days}`).then(checked(readEvaluation)),
    metrics: (days, hourBuckets = false) => get<unknown>(`/api/metrics?days=${days}${hourBuckets ? '&hourBuckets=1' : ''}`).then(checked(readMetrics)),
    activity: (org, team, since, signal) => get<unknown>(`/api/orgs/${segment(org)}/teams/${segment(team)}/activity?since=${encodeURIComponent(since)}`, signal).then(checked(readActivity)),
    outcomes: (org, days) => get<unknown>(`/api/orgs/${segment(org)}/outcomes?days=${days}`).then(checked(readOutcomes)),
    readings: (org) => get<unknown>(`/api/orgs/${segment(org)}/outcomes/readings`).then(checked(readReadings)),
    numberHistory: (org, kpi, limit) => get<unknown>(`/api/orgs/${segment(org)}/outcomes/observations?kpi=${segment(kpi)}&limit=${limit}`).then(checked(readNumberHistory)),
    enterNumber: (org, input) => post<unknown>(`/api/orgs/${segment(org)}/outcomes/observations`, input).then(checked(readNumberEntry)),
    effort: (org, days) => get<unknown>(`/api/orgs/${segment(org)}/effort?days=${days}`).then(checked(readEffort)),
    decisions: (limit) => get<DecisionLog>(`/api/decisions?limit=${limit}`),
    decideSigned: (org, team, { envelope, title, choiceLabel }) => {
      // The label travels only with a choice, and both notes are trimmed and cut for the log.
      const label = envelope.body.payload.choice === undefined || choiceLabel === undefined || forLog(choiceLabel) === '' ? {} : { choiceLabel: forLog(choiceLabel) }
      const input: SignedDecisionInput = { envelope, ...(title === undefined ? {} : { title: forLog(title) }), ...label }
      return post<DecisionResult>(`/api/orgs/${segment(org)}/teams/${segment(team)}/envelopes`, input)
    },
    registerPushToken: async (input) => void (await post<unknown>('/api/push/tokens', input)),
    vapidKey: () => get<VapidKey>('/api/push/vapid'),
    pushSubscriptions: () => get<PushSubscriptionList>('/api/push/subscriptions'),
    registerPushSubscription: (input) => post<PushSubscription>('/api/push/subscriptions', input),
    removePushSubscription: async (id) => void (await remove(`/api/push/subscriptions/${segment(id)}`)),
    devices: () => get<DeviceList>('/api/devices'),
    requestDevice: (input) => post<DeviceRequest>('/api/devices/requests', input),
    submitDeviceChange: (change) => post<DeviceChangeResult>('/api/devices/changes', change),
    redeemClaim: (input) => post<ClaimState>('/api/claim', input),
    claimState: () => get<ClaimState>('/api/claim'),
  }
}
