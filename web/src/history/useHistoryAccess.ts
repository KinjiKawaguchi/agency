import { useQueries, useQuery } from '@tanstack/react-query'
import { ApiError } from '../api/client'
import { useApi } from '../api/context'
import { aboutQuery, meQuery } from '../api/queries'
import { guestOf } from '../guest/provisional'

/**
 * Whether this viewer is the owner whose org's history this is, and the node says it has one. The node lists the `history` capability when it
 * serves it, so a node that does not is not asked; for one that does, the owner's read is the test (403 for anyone else, 404 for an org that is not
 * theirs), and the screen then shows nothing, not even that it exists. While it is being read the answer is not known, which is not "no", and a
 * failed read (a node that is down) is not "no" either: the owner must not lose the page in an outage, so that is `unknown`.
 */
export type HistoryAccess = { status: 'loading' } | { status: 'none' } | { status: 'unknown'; error: Error } | { status: 'available'; orgs: readonly string[] }

/** The node's answer to anyone who is not the owner, or has no such org: nothing to show, and nothing to say. */
export const isNotTheOwner = (error: unknown): boolean => error instanceof ApiError && (error.status === 403 || error.status === 404)

export function useHistoryAccess(): HistoryAccess {
  const api = useApi()
  const me = useQuery(meQuery(api))
  // A member the node hosts is never the owner: nothing is asked of the node for them, what it says of itself included.
  const isGuest = me.data !== undefined && guestOf(me.data) !== undefined
  const about = useQuery({ ...aboutQuery(api), enabled: me.data !== undefined && !isGuest })
  const hasHistory = about.data?.capabilities.includes('history') === true
  const orgs = !hasHistory || isGuest ? [] : (me.data?.orgs ?? []) /* stand-in-ok(routing): the orgs to ask about once the viewer is read; the status below says it is not known yet */
  // One entry of each org is the owner's test: the page itself is read by the screen, a page at a time, so that this does not ask for the whole first page of every org.
  const reads = useQueries({ queries: orgs.map((org) => ({ queryKey: ['history-access', org.id], queryFn: () => api.orgHistory(org.id, { limit: 1 }), staleTime: 60_000 })) })
  if (me.isPending) return { status: 'loading' }
  if (me.data === undefined && me.error !== null) return { status: 'unknown', error: me.error }
  if (isGuest) return { status: 'none' }
  if (about.isPending) return { status: 'loading' }
  if (about.data === undefined && about.error !== null) return { status: 'unknown', error: about.error }
  const readable = orgs.filter((_org, index) => reads[index]?.data !== undefined).map((org) => org.id)
  if (readable.length > 0) return { status: 'available', orgs: readable }
  if (reads.some((read) => read.isPending)) return { status: 'loading' }
  const failed = reads.flatMap((read) => (read.error === null ? [] : [read.error])).find((error) => !isNotTheOwner(error))
  return failed === undefined ? { status: 'none' } : { status: 'unknown', error: failed }
}
