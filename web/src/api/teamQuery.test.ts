import { QueryClient } from '@tanstack/react-query'
import { sampleTeamView } from './teamview'
import type { TeamSnapshot } from './clientApi'
import { ApiError, type ApiClient } from './client'
import { teamQuery, type TeamResult } from './queries'
import { createQueryClient } from '../app/queryClient'

const snapshot: TeamSnapshot = { view: sampleTeamView, viewer: 'owner', mine: [], waitingOnViewer: [], source: { kind: 'local' } }

/** An API whose team reads answer from a script: a status to fail with, or the snapshot. */
function apiAnswering(script: Array<number | 'ok'>): { api: ApiClient; calls: () => number } {
  let calls = 0
  const api = {
    teamSnapshot: () => {
      const next = script[Math.min(calls, script.length - 1)]
      calls += 1
      return next === 'ok' ? Promise.resolve(snapshot) : Promise.reject(new ApiError('The board has not been read yet.', next ?? 503))
    },
  } as unknown as ApiClient
  return { api, calls: () => calls }
}

const read = (client: QueryClient, api: ApiClient): Promise<TeamResult> => client.fetchQuery({ ...teamQuery(api, 'acme', 'app'), staleTime: 0 }) as Promise<TeamResult>

describe('a team read', () => {
  it('is tried again while the node says the view is not ready, and is ready once it is', async () => {
    const { api, calls } = apiAnswering([503, 503, 'ok'])
    const result = await read(createQueryClient([0, 0, 0]), api)
    expect(result.status).toBe('ready')
    expect(calls()).toBe(3)
  })

  it('is a failure, in the team’s place, only when the tries are used up', async () => {
    const { api, calls } = apiAnswering([503])
    const result = await read(createQueryClient([0, 0]), api)
    expect(result).toMatchObject({ status: 'failed', error: { message: 'The board has not been read yet.' } })
    expect(calls()).toBe(3)
  })

  it('does not try again an answer that will not change', async () => {
    const { api, calls } = apiAnswering([404])
    expect((await read(createQueryClient([0, 0]), api)).status).toBe('failed')
    expect(calls()).toBe(1)
  })

  it('keeps the view already shown when a refresh keeps failing, and shows the failure of a refusal', async () => {
    const client = createQueryClient([0])
    const good = apiAnswering(['ok'])
    expect((await read(client, good.api)).status).toBe('ready')
    const busy = apiAnswering([503])
    expect((await read(client, busy.api)).status).toBe('ready')
    expect(busy.calls()).toBe(2)
    const refused = apiAnswering([403])
    expect((await read(client, refused.api)).status).toBe('failed')
  })
})
