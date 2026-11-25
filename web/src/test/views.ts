import type { Team } from '../api/teams'
import { sampleTeamView, type TeamRequest, type TeamView } from '../api/teamview'

/** A local team whose viewer goes by `viewer` (the owner, unless said otherwise). `mine` is what the node lists as the viewer's, by default the requests addressed to them or to no one. */
export const teamOf = (view: TeamView, viewer = 'owner', mine?: readonly string[]): Team => ({
  view,
  viewer,
  source: { kind: 'local' },
  // The people whose `waitingOn` names the viewer, which is what the node lists.
  waitingOnViewer: new Set(view.people.filter((person) => person.waitingOn?.kind === 'person' && person.waitingOn.person === viewer).map((person) => person.id)),
  mine: new Set(mine ?? view.requests.filter((request) => (request.to ?? viewer) === viewer).map((request) => request.id)),
})

/** The shared sample with one more request, addressed to the person named `to`. */
export function withRequestFor(to: string): TeamView {
  const extra: TeamRequest = {
    id: 'r-20261124-120000-e5f6',
    kind: 'question',
    title: 'Should the nightly export include archived projects?',
    context: '',
    from: 'lead',
    to,
    createdAt: '2026-11-24T12:00:00Z',
    blocks: [],
    options: [
      { key: 'yes', text: 'Include them', isRecommended: false },
      { key: 'no', text: 'Leave them out', isRecommended: false },
    ],
    answerWith: { kind: 'answer', target: 'r-20261124-120000-e5f6' },
  }
  return { ...sampleTeamView, requests: [...sampleTeamView.requests, extra] }
}
