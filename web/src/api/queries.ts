import { queryOptions, useQueries, useQuery } from '@tanstack/react-query'
import type { ApiClient } from './client'
import { useApi } from './context'
import type { Me, OrgRef, TeamRef } from './clientApi'
import { toTeam, type Team } from './teams'

export const POLL_MS = 30_000

/** How often the device list is read while a screen that shows a device waiting is open. A hidden tab does not poll. */
export const DEVICES_LIVE_MS = 4_000

export const meQuery = (api: ApiClient) => queryOptions({ queryKey: ['me'], queryFn: () => api.me(), refetchInterval: POLL_MS })

export const TEAM_KEY = ['team'] as const

export const teamQuery = (api: ApiClient, org: string, team: string) =>
  queryOptions({ queryKey: [...TEAM_KEY, org, team], queryFn: () => api.teamSnapshot(org, team).then(toTeam), refetchInterval: POLL_MS })

export const DEVICES_KEY = ['devices'] as const

/** The account's devices. A device waiting to be added polls quickly, to notice when it is. */
export const devicesQuery = (api: ApiClient, pollMs: number | false = POLL_MS) =>
  queryOptions({ queryKey: DEVICES_KEY, queryFn: () => api.devices(), refetchInterval: pollMs })

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

/** The viewer and every team they can reach, loaded together. */
export function useTeams(): Teams {
  const api = useApi()
  const me = useQuery(meQuery(api))
  const refs = (me.data?.orgs ?? []).flatMap((org) => org.teams.map((team) => ({ org: org.id, team: team.id })))
  const results = useQueries({ queries: refs.map(({ org, team }) => teamQuery(api, org, team)) })
  return {
    me: me.data,
    teams: results.flatMap((result) => (result.data === undefined ? [] : [result.data])),
    isLoading: me.isPending || results.some((result) => result.isPending),
    error: me.error ?? results.find((result) => result.error !== null)?.error ?? null,
  }
}

/** What is known of one team: still loading, could not be loaded, or its snapshot. */
export type TeamState = { status: 'loading' } | { status: 'failed'; error: Error } | { status: 'ready'; team: Team }

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

/**
 * Every team of every org, each with its own outcome, so that a team that cannot be read is shown as that
 * in its place. It reads the same queries as `useTeams`, so the two share what the page has already loaded.
 */
export function useTeamStates(): TeamStates {
  const api = useApi()
  const me = useQuery(meQuery(api))
  const refs = (me.data?.orgs ?? []).flatMap((org) => org.teams.map((team) => ({ org: org.id, team: team.id })))
  const results = useQueries({ queries: refs.map(({ org, team }) => teamQuery(api, org, team)) })
  const stateOf = (org: string, team: string): TeamState => {
    const result = results[refs.findIndex((ref) => ref.org === org && ref.team === team)]
    if (result?.data !== undefined) return { status: 'ready', team: result.data }
    return result?.error ? { status: 'failed', error: result.error } : { status: 'loading' }
  }
  return {
    me: me.data,
    orgs: (me.data?.orgs ?? []).map((org) => ({ org, teams: org.teams.map((ref) => ({ ref, state: stateOf(org.id, ref.id) })) })),
    error: me.error,
  }
}
