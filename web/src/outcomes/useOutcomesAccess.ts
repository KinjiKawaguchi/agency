import { useQueries, useQuery } from '@tanstack/react-query'
import { useApi } from '../api/context'
import { meQuery, outcomesQuery } from '../api/queries'

/** The days the first read asks for, and the page opens on: a monthly figure usually has no number in 14 days. */
export const DEFAULT_DAYS = 90
export const OUTCOMES_PERIODS: readonly number[] = [30, 90]

/**
 * Whether this viewer is the owner whose numbers these are, found by reading them: the node answers 403 to anyone else
 * (a read included) and the page then shows nothing of it, not even that it exists. While that is being read the answer
 * is not known, which is not "no".
 */
export type OutcomesAccess = { status: 'loading' } | { status: 'none' } | { status: 'available'; orgs: readonly string[] }

export function useOutcomesAccess(): OutcomesAccess {
  const api = useApi()
  const me = useQuery(meQuery(api))
  const orgs = me.data?.orgs ?? [] /* stand-in-ok(routing): the orgs to ask about once the viewer is read; the status below says it is not known yet */
  const reads = useQueries({ queries: orgs.map((org) => outcomesQuery(api, org.id, DEFAULT_DAYS)) })
  if (me.isPending) return { status: 'loading' }
  const readable = orgs.filter((_org, index) => reads[index]?.data !== undefined).map((org) => org.id)
  if (readable.length > 0) return { status: 'available', orgs: readable }
  return reads.some((read) => read.isPending) ? { status: 'loading' } : { status: 'none' }
}
