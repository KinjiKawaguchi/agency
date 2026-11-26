import { looksLikeExpiredSignIn, SignInExpiredError } from './session'
import type { DecisionLog, DecisionResult, DeviceChange, DeviceChangeResult, DeviceList, DeviceRequest, DeviceRequestInput, Me, PushTokenInput, Signed, SignedDecisionInput, OrgDefinition, TeamSnapshot, TokenUsage } from './clientApi'
import type { Effort, Evaluation } from './evaluationApi'

export type FetchLike = (input: Request) => Promise<Response>

/**
 * An error from the node, carrying the status so callers can tell them apart: 404 is gone (or not served),
 * 409 is a refusal by the team's node, and a runtime's own 400 and 404 pass through unchanged.
 */
export class ApiError extends Error {
  readonly status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = 'ApiError'
    this.status = status
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
  /** Asks the node to add this device; a device that is already added confirms it after comparing the pairing codes. */
  requestDevice: (input: DeviceRequestInput) => Promise<DeviceRequest>
  /** Adds a device: a change signed by one of the account's devices. */
  submitDeviceChange: (change: Signed<DeviceChange>) => Promise<DeviceChangeResult>
  usage: (org: string, team: string, days: number) => Promise<TokenUsage>
  /** How fast each team of the org ships: merged pull requests, lead times, red main, reverts, cost. */
  evaluation: (org: string, days: number) => Promise<Evaluation>
  /** What each team of the org spent, by work type. */
  effort: (org: string, days: number) => Promise<Effort>
}

const segment = encodeURIComponent

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

/** The node puts a token for this page in its served index.html; every write sends it back. */
function readToken(): string {
  return document.querySelector('meta[name="agency-token"]')?.getAttribute('content') ?? ''
}

async function readJson<T>(response: Response): Promise<T> {
  const body: unknown = await response.json().catch(() => ({}))
  if (!response.ok) {
    const message = typeof body === 'object' && body !== null && 'error' in body ? String(body.error) : `HTTP ${response.status}`
    throw new ApiError(message, response.status)
  }
  return body as T
}

/** Talks to the home node. `send` is injectable so tests can route calls to the mock node. */
export function createApiClient(send: FetchLike = (request) => fetch(request), page: PageEnvironment = browserPage): ApiClient {
  const url = (path: string): URL => new URL(path, window.location.origin)
  const listeners = new Set<(state: SessionState) => void>()
  const tell = (state: SessionState): void => listeners.forEach((listener) => listener(state))

  /** Sends a call. One that fails without an answer, away from this machine, is taken for an expired sign-in. */
  const dispatch = async (request: Request): Promise<Response> => {
    let response: Response
    try {
      response = await send(request)
    } catch (cause) {
      if (!looksLikeExpiredSignIn({ hostname: page.hostname() }, page.isOnline())) throw cause
      tell('expired')
      throw new SignInExpiredError()
    }
    tell('ok')
    return response
  }
  const get = <T>(path: string): Promise<T> => dispatch(new Request(url(path), { headers: { accept: 'application/json' } })).then(readJson<T>)

  /** A write: JSON with the page's token, which the node demands of every write. */
  const post = <T>(path: string, body: unknown): Promise<T> =>
    dispatch(new Request(url(path), { method: 'POST', headers: { 'content-type': 'application/json', 'x-agency-token': readToken() }, body: JSON.stringify(body) })).then(readJson<T>)

  return {
    onSession: (listener) => {
      listeners.add(listener)
      return () => void listeners.delete(listener)
    },
    me: () => get<Me>('/api/me'),
    teamSnapshot: (org, team) => get<TeamSnapshot>(`/api/orgs/${segment(org)}/teams/${segment(team)}/view`),
    orgDefinition: (org) => get<OrgDefinition>(`/api/orgs/${segment(org)}`),
    usage: (org, team, days) => get<TokenUsage>(`/api/orgs/${segment(org)}/teams/${segment(team)}/usage?days=${days}`),
    evaluation: (org, days) => get<Evaluation>(`/api/orgs/${segment(org)}/evaluation?days=${days}`),
    effort: (org, days) => get<Effort>(`/api/orgs/${segment(org)}/effort?days=${days}`),
    decisions: (limit) => get<DecisionLog>(`/api/decisions?limit=${limit}`),
    decideSigned: (org, team, { envelope, title, choiceLabel }) => {
      // The label travels only with a choice, and both notes are trimmed and cut for the log.
      const label = envelope.body.payload.choice === undefined || choiceLabel === undefined || forLog(choiceLabel) === '' ? {} : { choiceLabel: forLog(choiceLabel) }
      const input: SignedDecisionInput = { envelope, ...(title === undefined ? {} : { title: forLog(title) }), ...label }
      return post<DecisionResult>(`/api/orgs/${segment(org)}/teams/${segment(team)}/envelopes`, input)
    },
    registerPushToken: async (input) => void (await post<unknown>('/api/push/tokens', input)),
    devices: () => get<DeviceList>('/api/devices'),
    requestDevice: (input) => post<DeviceRequest>('/api/devices/requests', input),
    submitDeviceChange: (change) => post<DeviceChangeResult>('/api/devices/changes', change),
  }
}
