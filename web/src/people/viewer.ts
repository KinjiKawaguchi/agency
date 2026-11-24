import type { TeamView } from '../api/teamview'

/**
 * The team runtime's name for the person the viewer answers for in this team.
 * The Client API will report it per team; until then it is the team's owner, and this is the one
 * place that guess lives.
 */
export function viewerNameOf(_view: TeamView): string {
  return 'owner'
}
