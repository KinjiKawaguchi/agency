import { sampleTeamView, type TeamRequest, type TeamView } from '../api/teamview'

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
