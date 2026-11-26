import { useQueries, useQuery } from '@tanstack/react-query'
import { ApiError } from '../api/client'
import { useApi } from '../api/context'
import { meQuery, outcomesQuery } from '../api/queries'

/** The days the first read asks for, and the page opens on: a monthly figure usually has no number in 14 days. */
export const DEFAULT_DAYS = 90
export const OUTCOMES_PERIODS: readonly number[] = [30, 90]

/**
 * Whether this viewer is the owner whose numbers these are, found by reading them: the node answers 403 to anyone else
 * (a read included) and 404 for an org that is not theirs, and the page then shows nothing of it, not even that it exists.
 * While that is being read the answer is not known, which is not "no", and neither is a read that failed (a node
 * that is down, a timeout, the viewer not read): the owner must not lose the page in an outage, so that is `unknown`.
 */
export type OutcomesAccess = { status: 'loading' } | { status: 'none' } | { status: 'unknown'; error: Error } | { status: 'available'; orgs: readonly string[] }

/** The node's answer to anyone who is not the owner of the numbers, or has no such org: nothing to show, and nothing to say. */
export const isNotTheOwner = (error: unknown): boolean => error instanceof ApiError && (error.status === 403 || error.status === 404)

export function useOutcomesAccess(): OutcomesAccess {
  const api = useApi()
  const me = useQuery(meQuery(api))
  const orgs = me.data?.orgs ?? [] /* stand-in-ok(routing): the orgs to ask about once the viewer is read; the status below says it is not known yet */
  const reads = useQueries({ queries: orgs.map((org) => outcomesQuery(api, org.id, DEFAULT_DAYS)) })
  if (me.isPending) return { status: 'loading' }
  if (me.error !== null) return { status: 'unknown', error: me.error }
  const readable = orgs.filter((_org, index) => reads[index]?.data !== undefined).map((org) => org.id)
  if (readable.length > 0) return { status: 'available', orgs: readable }
  if (reads.some((read) => read.isPending)) return { status: 'loading' }
  const failed = reads.flatMap((read) => (read.error === null ? [] : [read.error])).find((error) => !isNotTheOwner(error))
  return failed === undefined ? { status: 'none' } : { status: 'unknown', error: failed }
}
