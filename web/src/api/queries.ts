import { queryOptions, useQueries, useQuery } from '@tanstack/react-query'
import type { ApiClient } from './client'
import { useApi } from './context'
import type { Me } from './clientApi'
import { toTeam, type Team } from './teams'

export const POLL_MS = 30_000

export const meQuery = (api: ApiClient) => queryOptions({ queryKey: ['me'], queryFn: () => api.me(), refetchInterval: POLL_MS })

export const TEAM_KEY = ['team'] as const

export const teamQuery = (api: ApiClient, org: string, team: string) =>
  queryOptions({ queryKey: [...TEAM_KEY, org, team], queryFn: () => api.teamSnapshot(org, team).then(toTeam), refetchInterval: POLL_MS })

export const orgDefinitionQuery = (api: ApiClient, org: string) =>
  queryOptions({ queryKey: ['org-definition', org], queryFn: () => api.orgDefinition(org), staleTime: POLL_MS })

export const usageQuery = (api: ApiClient, org: string, team: string, days: number) =>
  queryOptions({ queryKey: ['usage', org, team, days], queryFn: () => api.usage(org, team, days), staleTime: 2 * POLL_MS })

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
