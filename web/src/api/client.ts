import type { DecisionAnswer, Me } from './clientApi'
import type { TeamView } from './teamview'

export type FetchLike = (input: Request) => Promise<Response>

/** An error from the node, carrying the status so callers can tell "gone" from "down". */
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
  teamView: (org: string, team: string) => Promise<TeamView>
  decide: (answer: DecisionAnswer) => Promise<void>
}

const segment = encodeURIComponent

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
  const get = <T>(path: string): Promise<T> =>
    send(new Request(new URL(path, window.location.origin), { headers: { accept: 'application/json' } })).then(readJson<T>)

  return {
    me: () => get<Me>('/api/me'),
    teamView: (org, team) => get<TeamView>(`/api/orgs/${segment(org)}/teams/${segment(team)}/view`),
    decide: ({ org, team, answerWith, choice, text }) =>
      send(
        new Request(new URL(`/api/orgs/${segment(org)}/teams/${segment(team)}/decisions`, window.location.origin), {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ ...answerWith, text, ...(choice === undefined ? {} : { choice }) }),
        }),
      ).then(readJson<unknown>).then(() => undefined),
  }
}
