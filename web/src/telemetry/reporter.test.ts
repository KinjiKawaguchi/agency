import { ApiError } from '../api/client'
import { SignInExpiredError } from '../api/session'
import { createReporter, PAUSE_MS } from './reporter'
import { createSampleQueue, type Sample } from './samples'

const sample = (ms: number): Sample => ({ route: 'GET /api/me', status: '2xx', ms })

/** A reporter over a queue, a clock the test moves, and a node that answers each report with the next of `answers` (an error, or nothing for taken). */
function reporting(...answers: Array<Error | undefined>) {
  const queue = createSampleQueue()
  const reports: Sample[][] = []
  let now = 1_000
  const remaining = [...answers]
  const reporter = createReporter({
    queue,
    now: () => now,
    report: async (samples) => {
      reports.push([...samples])
      const answer = remaining.shift()
      if (answer !== undefined) throw answer
    },
  })
  return { queue, reports, reporter, moveClock: (ms: number) => void (now += ms) }
}

describe('sending the waiting samples', () => {
  it('sends everything that waits as one report, oldest first, and nothing when nothing waits', async () => {
    const { queue, reports, reporter } = reporting()
    await reporter.flush()
    expect(reports).toEqual([])
    queue.add(sample(1))
    queue.add(sample(2))
    await reporter.flush()
    expect(reports).toEqual([[sample(1), sample(2)]])
    expect(queue.size()).toBe(0)
  })

  it('does not send a second report while one is on its way', async () => {
    const queue = createSampleQueue()
    const reports: Sample[][] = []
    let finish: () => void = () => undefined
    const reporter = createReporter({ queue, report: (samples) => new Promise<void>((resolve) => { reports.push([...samples]); finish = resolve }) })
    queue.add(sample(1))
    const first = reporter.flush()
    queue.add(sample(2))
    await reporter.flush()
    expect(reports).toEqual([[sample(1)]])
    finish()
    await first
    const second = reporter.flush()
    finish()
    await second
    expect(reports).toEqual([[sample(1)], [sample(2)]])
  })

  it.each([400, 413])('does not send again a report the node refused as it was made (%i): its samples are gone, the later ones go', async (status) => {
    const { queue, reports, reporter } = reporting(new ApiError('refused', status))
    queue.add(sample(1))
    await reporter.flush()
    queue.add(sample(2))
    await reporter.flush()
    expect(reports).toEqual([[sample(1)], [sample(2)]])
    expect(reporter.hasStopped()).toBe(false)
  })

  it.each([403, 404, 405])('stops for the page when the node has no such route or does not take this viewer’s reports (%i), and forgets what waits', async (status) => {
    const { queue, reports, reporter, moveClock } = reporting(new ApiError('no', status))
    queue.add(sample(1))
    await reporter.flush()
    expect(reporter.hasStopped()).toBe(true)
    queue.add(sample(2))
    moveClock(PAUSE_MS * 10)
    await reporter.flush()
    expect(reports).toEqual([[sample(1)]])
  })

  it('forgets the samples that came while the report that made it stop was on its way', async () => {
    const queue = createSampleQueue()
    let refuse: (error: Error) => void = () => undefined
    const reporter = createReporter({ queue, report: () => new Promise<void>((_taken, rejected) => void (refuse = rejected)) })
    queue.add(sample(1))
    const sending = reporter.flush()
    queue.add(sample(2))
    refuse(new ApiError('no', 403))
    await sending
    expect(reporter.hasStopped()).toBe(true)
    expect(queue.size()).toBe(0)
  })

  it.each([
    ['a busy node (429)', new ApiError('slow down', 429)],
    ['a failing node (503)', new ApiError('busy', 503)],
    ['no answer', new TypeError('Failed to fetch')],
    ['a sign-in that ended', new SignInExpiredError()],
  ])('keeps the samples and waits a minute after %s, then sends them with the newer ones', async (_name, error) => {
    const { queue, reports, reporter, moveClock } = reporting(error)
    queue.add(sample(1))
    await reporter.flush()
    queue.add(sample(2))
    moveClock(PAUSE_MS - 1)
    await reporter.flush()
    expect(reports).toEqual([[sample(1)]])
    moveClock(1)
    await reporter.flush()
    expect(reports).toEqual([[sample(1)], [sample(1), sample(2)]])
  })

  it('keeps only the newest samples within the limit when a report is tried again after a long failure', async () => {
    const queue = createSampleQueue(3)
    const reports: Sample[][] = []
    let now = 0
    const reporter = createReporter({ queue, now: () => now, report: async (samples) => {
        reports.push([...samples])
        throw new TypeError('Failed to fetch')
      },
    })
    for (const ms of [1, 2, 3]) queue.add(sample(ms))
    await reporter.flush()
    for (const ms of [4, 5]) queue.add(sample(ms))
    now += PAUSE_MS
    await reporter.flush()
    expect(reports[1]).toEqual([sample(3), sample(4), sample(5)])
  })
})
