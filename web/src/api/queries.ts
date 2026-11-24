import { queryOptions, useQueries, useQuery } from '@tanstack/react-query'
import type { ApiClient } from './client'
import { useApi } from './context'
import type { Me } from './clientApi'
import type { TeamView } from './teamview'

export const POLL_MS = 30_000

export const meQuery = (api: ApiClient) => queryOptions({ queryKey: ['me'], queryFn: () => api.me(), staleTime: POLL_MS })

export const teamViewQuery = (api: ApiClient, org: string, team: string) =>
  queryOptions({ queryKey: ['team-view', org, team], queryFn: () => api.teamView(org, team), refetchInterval: POLL_MS })

export const TEAM_VIEW_KEY = ['team-view'] as const

export interface TeamViews {
  me: Me | undefined
  views: readonly TeamView[]
  isLoading: boolean
  error: Error | null
}

/** The viewer and the views of every team they can reach, loaded together. */
export function useTeamViews(): TeamViews {
  const api = useApi()
  const me = useQuery(meQuery(api))
  const teams = (me.data?.orgs ?? []).flatMap((org) => org.teams.map((team) => ({ org: org.id, team: team.id })))
  const results = useQueries({ queries: teams.map(({ org, team }) => teamViewQuery(api, org, team)) })
  const views = results.flatMap((result) => (result.data === undefined ? [] : [result.data]))
  const failed = results.find((result) => result.error !== null)?.error ?? null
  return {
    me: me.data,
    views,
    isLoading: me.isPending || results.some((result) => result.isPending),
    error: me.error ?? failed,
  }
}
