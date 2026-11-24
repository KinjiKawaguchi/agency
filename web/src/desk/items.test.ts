import { sampleTeamView } from '../api/teamview'
import { withRequestFor } from '../test/views'
import { blockedPeople, canDecide, openItems, resolveSelection, situationOf, stepSelection } from './items'

const views = [sampleTeamView]

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
    const items = openItems([withRequestFor('frontend')])
    expect(items.map((item) => item.group)).toEqual(['proposal', 'question', 'pendingDecision', 'others'])
    expect(items.at(-1)?.decider?.label).toBe('Frontend')
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
    const others = openItems([withRequestFor('frontend')]).find((item) => item.group === 'others')
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
