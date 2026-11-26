import { useQuery } from '@tanstack/react-query'
import { useApi } from '../api/context'
import { meQuery } from '../api/queries'
import { guestOf } from './provisional'

/**
 * What the node says the viewer is to it: a member hosted by the node (a guest), anyone else, or not known yet. A read that
 * failed with nothing earlier to go by is `unknown`, and is treated as anyone else: the pages then show their own failures.
 */
export type Standing = { status: 'loading' } | { status: 'unknown' } | { status: 'guest'; member?: string } | { status: 'member' }

export function useStanding(): Standing {
  const me = useQuery(meQuery(useApi()))
  // An answer already held stands even when a later read of it fails: only a read with nothing to show is a failure.
  if (me.data !== undefined) {
    const guest = guestOf(me.data)
    return guest === undefined ? { status: 'member' } : { status: 'guest', ...guest }
  }
  // A read started again after a failure shows as pending with no error, so what has failed before is counted: otherwise the
  // pages opened for the failure would be taken away by the read they start, and opened again by the next failure.
  return me.error === null && me.errorUpdateCount === 0 ? { status: 'loading' } : { status: 'unknown' }
}

/** The pages a guest has: their decisions, and their devices. */
const GUEST_PAGES = ['/desk', '/devices'] as const

export const isGuestPage = (path: string): boolean => GUEST_PAGES.some((page) => path === page || path.startsWith(`${page}/`))
