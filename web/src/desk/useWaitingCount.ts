import { useTeams } from '../api/queries'
import { countWaitingOnOwner } from './items'

/** How many requests wait for the viewer across every team: still being read, not known (a team could not be read), or the node's answer. */
export type Waiting = { status: 'loading' } | { status: 'unknown' } | { status: 'known'; count: number }

export function useWaiting(): Waiting {
  const { me, teams, isLoading, error } = useTeams()
  if (me === undefined || isLoading) return error === null ? { status: 'loading' } : { status: 'unknown' }
  return error === null ? { status: 'known', count: countWaitingOnOwner(teams) } : { status: 'unknown' }
}

/**
 * How many requests wait for the viewer across every team, or undefined while that is not known for
 * certain: the account or a team is still loading, or a team could not be read.
 */
export function useWaitingCount(): number | undefined {
  const waiting = useWaiting()
  return waiting.status === 'known' ? waiting.count : undefined
}
