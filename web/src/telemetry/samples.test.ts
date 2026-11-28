import { createSampleQueue, MAX_SAMPLES_PER_REPORT, statusOf, wholeMs, type Sample } from './samples'

const sample = (n: number): Sample => ({ route: 'GET /api/me', status: '2xx', ms: n })

describe('the class of an answer', () => {
  it('is its hundred, and nothing for a status that is not an answer of the node', () => {
    expect([200, 204, 299].map(statusOf)).toEqual(['2xx', '2xx', '2xx'])
    expect([300, 399].map(statusOf)).toEqual(['3xx', '3xx'])
    expect([400, 404, 499].map(statusOf)).toEqual(['4xx', '4xx', '4xx'])
    expect([500, 503, 599].map(statusOf)).toEqual(['5xx', '5xx', '5xx'])
    expect([0, 100, 199, 600, -1].map(statusOf)).toEqual([undefined, undefined, undefined, undefined, undefined])
  })
})

describe('the milliseconds a call took', () => {
  it('are whole, never negative, and no more than the node tells apart', () => {
    expect([0, 0.4, 0.5, 12.49, 12.5].map(wholeMs)).toEqual([0, 0, 1, 12, 13])
    expect(wholeMs(-3)).toBe(0)
    expect(wholeMs(30_000)).toBe(30_000)
    expect(wholeMs(90_000)).toBe(30_001)
  })
})

describe('the samples that wait for the next report', () => {
  it('are taken oldest first, and taking leaves none', () => {
    const queue = createSampleQueue()
    queue.add(sample(1))
    queue.add(sample(2))
    expect(queue.size()).toBe(2)
    expect(queue.take().map((one) => one.ms)).toEqual([1, 2])
    expect(queue.size()).toBe(0)
    expect(queue.take()).toEqual([])
  })

  it('keep the newest 200 and drop the older, which are never a second request', () => {
    const queue = createSampleQueue()
    for (let n = 1; n <= MAX_SAMPLES_PER_REPORT + 5; n++) queue.add(sample(n))
    const taken = queue.take()
    expect(taken).toHaveLength(MAX_SAMPLES_PER_REPORT)
    expect(taken[0]?.ms).toBe(6)
    expect(taken[taken.length - 1]?.ms).toBe(MAX_SAMPLES_PER_REPORT + 5)
  })

  it('can be cleared, and do not change a list that was taken', () => {
    const queue = createSampleQueue()
    queue.add(sample(1))
    const taken = queue.take()
    queue.add(sample(2))
    expect(taken.map((one) => one.ms)).toEqual([1])
    queue.clear()
    expect(queue.size()).toBe(0)
  })
})
