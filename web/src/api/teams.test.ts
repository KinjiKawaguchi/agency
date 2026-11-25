import { sampleTeamView } from './teamview'
import { toTeam } from './teams'

describe('toTeam', () => {
  it('takes the viewer’s name from the snapshot', () => {
    expect(toTeam({ view: sampleTeamView, viewer: 'lead', mine: [], waitingOnViewer: [], source: { kind: 'local' } }).viewer).toBe('lead')
  })

  it('leaves the viewer absent when the node reports none, since the team does not address them', () => {
    const team = toTeam({ view: sampleTeamView, mine: [], waitingOnViewer: [], source: { kind: 'local' } })
    expect(team.viewer).toBeUndefined()
    expect(team.waitingOnViewer.size).toBe(0)
  })

  it('takes who waits on the viewer from the node, as a set of person ids', () => {
    expect(toTeam({ view: sampleTeamView, viewer: 'owner', mine: [], waitingOnViewer: ['backend'], source: { kind: 'local' } }).waitingOnViewer).toEqual(new Set(['backend']))
  })

  it('keeps where the copy came from', () => {
    const source = { kind: 'replica', fetchedAt: '2026-11-25T00:00:00Z', triedAt: '2026-11-25T00:01:00Z', error: 'refused' } as const
    expect(toTeam({ view: sampleTeamView, mine: [], waitingOnViewer: [], source }).source).toEqual(source)
  })
})
