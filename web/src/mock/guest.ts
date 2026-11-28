// A hosted member as the node treats them (design on #196, section 4): the team views are the answerer's, and nothing of
// the owner is served. Every name is made up.
import type { TeamView } from '../api/teamview'

export interface MockGuest {
  /** The member's id in the org. */
  member: string
  label: string
}

/** What a team's node serves an answerer: only the requests addressed to them, no people, work items, or epics. */
export function answererView(view: TeamView, guest: MockGuest): TeamView {
  return {
    ...view,
    people: [],
    work: [],
    epics: [],
    requests: view.requests.filter((request) => (request.to ?? 'owner') === 'owner').map((request) => ({ ...request, to: guest.member })),
  }
}

const ORG_ROUTE = /^\/api\/orgs\/[^/]+(\/(evaluation|effort|history)|\/outcomes(\/readings|\/observations)?)?$/
const TEAM_OWNER_ROUTE = /^\/api\/orgs\/[^/]+\/teams\/[^/]+\/(usage|activity)$/

/** The routes that are the owner's: a hosted member has no owner action, and no read beyond their seat. */
export const isRefusedToGuest = (pathname: string): boolean => ORG_ROUTE.test(pathname) || TEAM_OWNER_ROUTE.test(pathname) || pathname === '/api/push/tokens' || pathname.startsWith('/api/push/tokens/')
