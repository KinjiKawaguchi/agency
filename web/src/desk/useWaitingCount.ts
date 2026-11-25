import { useTeams } from '../api/queries'
import { countWaitingOnOwner } from './items'

/**
 * How many requests wait for the viewer across every team, or undefined while that is not known for
 * certain: the account or a team is still loading, or a team could not be read.
 */
export function useWaitingCount(): number | undefined {
  const { me, teams, isLoading, error } = useTeams()
  return me === undefined || isLoading || error !== null ? undefined : countWaitingOnOwner(teams)
}
