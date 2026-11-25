import type { DecisionAnswer, DecisionInput, DecisionResult, Me, OrgDefinition, TeamSnapshot, UsageReport } from './clientApi'

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

export interface ApiClient {
  me: () => Promise<Me>
  teamSnapshot: (org: string, team: string) => Promise<TeamSnapshot>
  decide: (answer: DecisionAnswer) => Promise<DecisionResult>
  orgDefinition: (org: string) => Promise<OrgDefinition>
  usage: (org: string, team: string, days: number) => Promise<UsageReport>
}

const segment = encodeURIComponent

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
export function createApiClient(send: FetchLike = (request) => fetch(request)): ApiClient {
  const url = (path: string): URL => new URL(path, window.location.origin)
  const get = <T>(path: string): Promise<T> => send(new Request(url(path), { headers: { accept: 'application/json' } })).then(readJson<T>)

  return {
    me: () => get<Me>('/api/me'),
    teamSnapshot: (org, team) => get<TeamSnapshot>(`/api/orgs/${segment(org)}/teams/${segment(team)}/view`),
    orgDefinition: (org) => get<OrgDefinition>(`/api/orgs/${segment(org)}`),
    usage: (org, team, days) => get<UsageReport>(`/api/orgs/${segment(org)}/teams/${segment(team)}/usage?days=${days}`),
    decide: ({ org, team, answerWith, choice, text }) => {
      const input: DecisionInput = { ...answerWith, text, ...(choice === undefined ? {} : { choice }) }
      return send(
        new Request(url(`/api/orgs/${segment(org)}/teams/${segment(team)}/decisions`), {
          method: 'POST',
          headers: { 'content-type': 'application/json', 'x-agency-token': readToken() },
          body: JSON.stringify(input),
        }),
      ).then(readJson<DecisionResult>)
    },
  }
}
