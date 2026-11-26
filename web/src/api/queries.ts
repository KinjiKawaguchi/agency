import { queryOptions, useQueries, useQuery, useSuspenseQuery } from '@tanstack/react-query'
import type { ApiClient } from './client'
import { useApi } from './context'
import type { Me, OrgRef, TeamRef } from './clientApi'
import { isRetryable, RETRY_DELAYS_MS, withRetry } from './retry'
import { toTeam, type Team } from './teams'

export const POLL_MS = 30_000

/** How often the device list is read while a screen that shows a device waiting is open. A hidden tab does not poll. */
export const DEVICES_LIVE_MS = 4_000

export const meQuery = (api: ApiClient) => queryOptions({ queryKey: ['me'], queryFn: () => api.me(), refetchInterval: POLL_MS })

export const TEAM_KEY = ['team'] as const

/** What is known of one team: still loading, could not be loaded (and when it gave up), or its snapshot. */
export type TeamState = { status: 'loading' } | { status: 'failed'; error: Error; at: number } | { status: 'ready'; team: Team }

/** What a read of a team answers: a team that could not be read is an answer, so that the others are still shown. */
export type TeamResult = Exclude<TeamState, { status: 'loading' }>

const retryDelaysOf = (meta: Record<string, unknown> | undefined): readonly number[] =>
  Array.isArray(meta?.retryDelays) ? (meta.retryDelays as number[]) : RETRY_DELAYS_MS

/**
 * One read of a team's view. A retryable failure (the node answers 503 while the view is not ready) is
 * tried again with backoff, and the query stays pending meanwhile: it is loading, not failed. When the
 * tries are used up the failure is the answer, unless there is an earlier view to keep showing; the next poll
 * tries again. An aborted read throws, as every read does.
 */
export const teamQuery = (api: ApiClient, org: string, team: string) =>
  queryOptions({
    queryKey: [...TEAM_KEY, org, team],
    queryFn: async ({ client, queryKey, signal, meta }): Promise<TeamResult> => {
      try {
        return { status: 'ready', team: toTeam(await withRetry(() => api.teamSnapshot(org, team), retryDelaysOf(meta), signal)) }
      } catch (error) {
        if (signal.aborted) throw error
        const previous = client.getQueryData<TeamResult>(queryKey)
        // A refresh that fails keeps the view already shown; only a first read, or a final refusal, shows the failure.
        if (previous?.status === 'ready' && isRetryable(error)) return previous
        return { status: 'failed', error: error instanceof Error ? error : new Error(String(error)), at: Date.now() }
      }
    },
    // Fresh for half a poll: a region that has just suspended on this read must not read it again as it mounts.
    staleTime: POLL_MS / 2,
    refetchInterval: POLL_MS,
    retry: false,
  })

export const DEVICES_KEY = ['devices'] as const

/** How often the device list is read: `pollMs`, unless the client was made with another (a test's). */
export const devicesPoll = (query: { meta?: Record<string, unknown> | undefined }, pollMs: number | false): number | false =>
  typeof query.meta?.devicesPollMs === 'number' ? query.meta.devicesPollMs : pollMs

/** The account's devices. A device waiting to be added polls quickly, to notice when it is. */
export const devicesQuery = (api: ApiClient, pollMs: number | false = POLL_MS) =>
  queryOptions({ queryKey: DEVICES_KEY, queryFn: () => api.devices(), refetchInterval: (query) => devicesPoll(query, pollMs) })

export const DECISIONS_KEY = ['decisions'] as const

export const RECENT_DECISIONS = 20

export const decisionsQuery = (api: ApiClient) =>
  queryOptions({ queryKey: [...DECISIONS_KEY, RECENT_DECISIONS], queryFn: () => api.decisions(RECENT_DECISIONS), refetchInterval: POLL_MS })

export const orgDefinitionQuery = (api: ApiClient, org: string) =>
  queryOptions({ queryKey: ['org-definition', org], queryFn: () => api.orgDefinition(org), staleTime: POLL_MS })

export const usageQuery = (api: ApiClient, org: string, team: string, days: number) =>
  queryOptions({ queryKey: ['usage', org, team, days], queryFn: () => api.usage(org, team, days), staleTime: 2 * POLL_MS })

export const evaluationQuery = (api: ApiClient, org: string, days: number) =>
  queryOptions({ queryKey: ['evaluation', org, days], queryFn: () => api.evaluation(org, days), staleTime: 2 * POLL_MS })

export const effortQuery = (api: ApiClient, org: string, days: number) =>
  queryOptions({ queryKey: ['effort', org, days], queryFn: () => api.effort(org, days), staleTime: 2 * POLL_MS })

export interface Teams {
  me: Me | undefined
  teams: readonly Team[]
  isLoading: boolean
  error: Error | null
}

/** The viewer and every team they can reach, loaded together. A team that could not be read is left out and named in `error`. */
export function useTeams(): Teams {
  const api = useApi()
  const me = useQuery(meQuery(api))
  const refs = (me.data?.orgs ?? []).flatMap((org) => org.teams.map((team) => ({ org: org.id, team: team.id })))
  const results = useQueries({ queries: refs.map(({ org, team }) => teamQuery(api, org, team)) })
  const failed = results.find((result) => result.data?.status === 'failed')?.data
  return {
    me: me.data,
    teams: results.flatMap((result) => (result.data?.status === 'ready' ? [result.data.team] : [])),
    isLoading: me.isPending || results.some((result) => result.isPending),
    error: me.error ?? results.find((result) => result.error !== null)?.error ?? (failed?.status === 'failed' ? failed.error : null),
  }
}

export interface OrgStates {
  org: OrgRef
  teams: ReadonlyArray<{ ref: TeamRef; state: TeamState }>
}

export interface TeamStates {
  me: Me | undefined
  orgs: readonly OrgStates[]
  /** The viewer's own account could not be read; nothing else can be shown. */
  error: Error | null
}

const refsOf = (me: Me | undefined): Array<{ org: string; team: string }> => (me?.orgs ?? []).flatMap((org) => org.teams.map((team) => ({ org: org.id, team: team.id })))

function statesOf(me: Me | undefined, results: ReadonlyArray<{ data: TeamResult | undefined; error: Error | null }>): readonly OrgStates[] {
  const refs = refsOf(me)
  const stateOf = (org: string, team: string): TeamState => {
    const result = results[refs.findIndex((ref) => ref.org === org && ref.team === team)]
    if (result?.data !== undefined) return result.data
    return result?.error ? { status: 'failed', error: result.error, at: Date.now() } : { status: 'loading' }
  }
  return (me?.orgs ?? []).map((org) => ({ org, teams: org.teams.map((ref) => ({ ref, state: stateOf(org.id, ref.id) })) }))
}

/**
 * Every team of every org, each with its own outcome, so that a team that cannot be read is shown as that
 * in its place. It reads the same queries as `useTeams`, so the two share what the page has already loaded.
 * `inBackground` keeps this reading's timer running while the tab is hidden (a hidden tab otherwise does not poll).
 */
export function useTeamStates({ inBackground = false }: { inBackground?: boolean } = {}): TeamStates {
  const api = useApi()
  const me = useQuery(meQuery(api))
  const results = useQueries({ queries: refsOf(me.data).map(({ org, team }) => ({ ...teamQuery(api, org, team), refetchIntervalInBackground: inBackground })) })
  return { me: me.data, orgs: statesOf(me.data, results), error: me.error }
}

/**
 * The same, for a region under a Suspense boundary: it suspends only until the viewer has been read. Each team is
 * then `loading` (it is read, and tried again while the node says it is not ready: its place shows a skeleton),
 * `failed` (it kept failing: its place says so), or `ready`, so that one team that is slow never holds the
 * others back. Only the viewer's own account failing throws, to the region's error boundary.
 */
export function useSuspenseTeamStates(): { me: Me; orgs: readonly OrgStates[]; teams: readonly Team[]; loading: ReadonlySet<string> } {
  const api = useApi()
  const { data: me } = useSuspenseQuery(meQuery(api))
  const results = useQueries({ queries: refsOf(me).map(({ org, team }) => teamQuery(api, org, team)) })
  const orgs = statesOf(me, results)
  const loading = new Set(orgs.flatMap(({ org, teams }) => teams.flatMap(({ ref, state }) => (state.status === 'loading' ? [`${org.id}/${ref.id}`] : []))))
  return { me, orgs, teams: orgs.flatMap(({ teams }) => teams.flatMap(({ state }) => (state.status === 'ready' ? [state.team] : []))), loading }
}
