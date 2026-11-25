import { sampleTeamView } from './teamview'
import { toTeam } from './teams'

describe('toTeam', () => {
  it('takes the viewer’s name from the snapshot', () => {
    expect(toTeam({ view: sampleTeamView, viewer: 'lead', mine: [], source: { kind: 'local' } }).viewer).toBe('lead')
  })

  it('takes the viewer to be the owner until the node says otherwise', () => {
    expect(toTeam({ view: sampleTeamView, mine: [], source: { kind: 'local' } }).viewer).toBe('owner')
  })

  it('keeps where the copy came from', () => {
    const source = { kind: 'replica', fetchedAt: '2026-11-25T00:00:00Z', triedAt: '2026-11-25T00:01:00Z', error: 'refused' } as const
    expect(toTeam({ view: sampleTeamView, mine: [], source }).source).toEqual(source)
  })
})
