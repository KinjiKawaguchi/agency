import type { TeamSnapshot } from '../api/clientApi'
import { toTeam } from '../api/teams'
import { sampleTeamView } from '../api/teamview'
import { teamOf, withRequestFor } from '../test/views'
import { blockedPeople, canDecide, openItems, resolveSelection, situationOf, stepSelection } from './items'

const views = [teamOf(sampleTeamView)]

describe('openItems', () => {
  it('orders by group, then by how many people are stopped, then oldest first', () => {
    expect(openItems(views).map((item) => [item.group, item.request.title])).toEqual([
      ['proposal', 'Add a second frontend agent'],
      ['question', 'Which database should the app use in production?'],
      ['pendingDecision', 'Drop support for the old export format?'],
    ])
  })

  it('treats an absent addressee as the owner', () => {
    expect(openItems(views).every((item) => item.isMine)).toBe(true)
  })

  it('puts requests addressed to someone else in their own group, last', () => {
    const items = openItems([teamOf(withRequestFor('frontend'))])
    expect(items.map((item) => item.group)).toEqual(['proposal', 'question', 'pendingDecision', 'others'])
    expect(items.at(-1)?.decider?.label).toBe('Frontend')
  })
})

describe('the viewer in a team', () => {
  it('is whoever the node names, so a request with no addressee is theirs', () => {
    const asLead = openItems([teamOf(sampleTeamView, 'lead')])
    // The proposal names the owner explicitly, so it now belongs to someone else.
    expect(asLead.map((item) => [item.request.title, item.isMine])).toEqual([
      ['Which database should the app use in production?', true],
      ['Drop support for the old export format?', true],
      ['Add a second frontend agent', false],
    ])
  })
})

describe('whose a request is', () => {
  const ids = sampleTeamView.requests.map((request) => request.id)

  it('is the node’s call: what it lists as the viewer’s is theirs, whatever `to` says', () => {
    // A request addressed to a runtime name the desk has never heard of, which the node gives to the owner.
    const view = withRequestFor('someone-unmapped')
    const extra = view.requests.at(-1)?.id ?? ''
    const items = openItems([teamOf(view, 'owner', [...ids, extra])])
    expect(items.every((item) => item.isMine)).toBe(true)
    expect(items.find((item) => item.request.id === extra)?.group).toBe('question')
  })

  it('does not take a request for the viewer’s because `to` is absent, when the node does not list it', () => {
    // A team where the viewer is not the owner: the node leaves the unaddressed requests out of `mine`.
    const items = openItems([teamOf(sampleTeamView, 'lead', [])])
    expect(items.map((item) => item.isMine)).toEqual([false, false, false])
    expect(items.map((item) => item.group)).toEqual(['others', 'others', 'others'])
    expect(items.every((item) => !canDecide(item))).toBe(true)
  })

  it('lists exactly what the node lists, and none from a node that sends no `mine`, so nothing is decidable on a guess', () => {
    expect(toTeam({ view: sampleTeamView, viewer: 'owner', mine: [], waitingOnViewer: [], source: { kind: 'local' } }).mine.size).toBe(0)
    const oldNode = { view: sampleTeamView, viewer: 'owner', source: { kind: 'local' } } as unknown as TeamSnapshot
    expect(toTeam(oldNode).mine.size).toBe(0)
    expect(toTeam({ view: sampleTeamView, viewer: 'owner', mine: ['x'], waitingOnViewer: [], source: { kind: 'local' } }).mine).toEqual(new Set(['x']))
  })
})

describe('a team that does not address the viewer', () => {
  it('has nobody waiting on them, even if `waitingOn` names the owner, and nothing is theirs', () => {
    const team = { ...teamOf(sampleTeamView), viewer: undefined, mine: new Set<string>(), waitingOnViewer: new Set<string>() }
    expect(situationOf([team], openItems([team])).waitingOnYou).toBe(0)
    expect(openItems([team]).every((item) => !item.isMine)).toBe(true)
  })

  it('counts the people the node lists as waiting on the viewer', () => {
    const team = teamOf(sampleTeamView)
    expect(situationOf([team], openItems([team])).waitingOnYou).toBe(1)
    expect(situationOf([{ ...team, waitingOnViewer: new Set(['backend', 'lead']) }], []).waitingOnYou).toBe(2)
  })
})

describe('blockedPeople', () => {
  it('names the people who wait, not the work items', () => {
    const question = openItems(views).find((item) => item.group === 'question')
    expect(question?.request.blocks).toEqual(['backend', 'app#12'])
    expect(question && blockedPeople(question).map((person) => person.label)).toEqual(['Backend'])
  })
})

describe('canDecide', () => {
  it('allows the owner to decide what is addressed to them', () => {
    expect(openItems(views).every(canDecide)).toBe(true)
  })

  it('refuses a request addressed to someone else', () => {
    const others = openItems([teamOf(withRequestFor('frontend'))]).find((item) => item.group === 'others')
    expect(others && canDecide(others)).toBe(false)
  })
})

describe('situationOf', () => {
  it('counts the other people, not the owner', () => {
    expect(situationOf(views, openItems(views))).toEqual({ waitingOnYou: 1, working: 2, total: 4, mine: 3 })
  })
})

describe('selection', () => {
  const items = openItems(views)

  it('keeps a selection that is still on the desk and otherwise opens the first item', () => {
    expect(resolveSelection(items, items[2]?.key)).toBe(items[2]?.key)
    expect(resolveSelection(items, 'gone')).toBe(items[0]?.key)
    expect(resolveSelection([], 'gone')).toBeUndefined()
  })

  it('steps and stops at both ends', () => {
    expect(stepSelection(items, items[0]?.key, -1)).toBe(items[0]?.key)
    expect(stepSelection(items, items[0]?.key, 1)).toBe(items[1]?.key)
    expect(stepSelection(items, items[2]?.key, 1)).toBe(items[2]?.key)
  })
})
