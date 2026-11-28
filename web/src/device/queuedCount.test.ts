import { MOST_DECISIONS } from '../api/queries'
import { queuedCount } from './queuedCount'

const entries = (queued: number, others: number) => [
  ...Array.from({ length: queued }, () => ({ state: { kind: 'queued' } })),
  ...Array.from({ length: others }, () => ({ state: { kind: 'applied' } })),
]

describe('how many of the decision log the node holds undelivered', () => {
  it('counts the queued ones, and says it is exact when the node gave fewer than it can', () => {
    expect(queuedCount(entries(3, 5))).toEqual({ count: 3, isAtLeast: false })
    expect(queuedCount([])).toEqual({ count: 0, isAtLeast: false })
    expect(queuedCount(entries(2, MOST_DECISIONS - 3))).toEqual({ count: 2, isAtLeast: false })
  })

  it('says it is at least that many when the node gave all it gives, since older ones may be left out', () => {
    expect(queuedCount(entries(4, MOST_DECISIONS - 4))).toEqual({ count: 4, isAtLeast: true })
    expect(queuedCount(entries(MOST_DECISIONS, 0))).toEqual({ count: MOST_DECISIONS, isAtLeast: true })
  })

  it('is 200, the most the node gives in one read', () => {
    expect(MOST_DECISIONS).toBe(200)
  })
})
