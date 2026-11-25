import { looksLikeExpiredSignIn, SignInExpiredError } from './session'
import type { DecisionAnswer, DecisionInput, DecisionLog, DecisionResult, Me, OrgDefinition, TeamSnapshot, TokenUsage } from './clientApi'

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
  decide: (answer: DecisionAnswer) => Promise<DecisionResult>
  orgDefinition: (org: string) => Promise<OrgDefinition>
  /** The account's recent decisions, newest first. */
  decisions: (limit: number) => Promise<DecisionLog>
  usage: (org: string, team: string, days: number) => Promise<TokenUsage>
}

const segment = encodeURIComponent

/** The longest title or option label the node keeps in its decision log. */
const MAX_LOGGED_TEXT = 200

const forLog = (text: string): string => [...text.trim()].slice(0, MAX_LOGGED_TEXT).join('')

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

  return {
    onSession: (listener) => {
      listeners.add(listener)
      return () => void listeners.delete(listener)
    },
    me: () => get<Me>('/api/me'),
    teamSnapshot: (org, team) => get<TeamSnapshot>(`/api/orgs/${segment(org)}/teams/${segment(team)}/view`),
    orgDefinition: (org) => get<OrgDefinition>(`/api/orgs/${segment(org)}`),
    usage: (org, team, days) => get<TokenUsage>(`/api/orgs/${segment(org)}/teams/${segment(team)}/usage?days=${days}`),
    decisions: (limit) => get<DecisionLog>(`/api/decisions?limit=${limit}`),
    decide: ({ org, team, answerWith, choice, choiceLabel, text, title }) => {
      // The node refuses a label that comes without a choice, so the two travel together or not at all.
      const chosen = choice === undefined ? {} : { choice, ...(choiceLabel === undefined || forLog(choiceLabel) === '' ? {} : { choiceLabel: forLog(choiceLabel) }) }
      const input: DecisionInput = { ...answerWith, text, title: forLog(title), ...chosen }
      return dispatch(
        new Request(url(`/api/orgs/${segment(org)}/teams/${segment(team)}/decisions`), {
          method: 'POST',
          headers: { 'content-type': 'application/json', 'x-agency-token': readToken() },
          body: JSON.stringify(input),
        }),
      ).then(readJson<DecisionResult>)
    },
  }
}
