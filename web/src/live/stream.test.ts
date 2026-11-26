import type { Activity, ActivityEvent } from '../api/activityApi'
import { ApiError } from '../api/client'
import { ActivityStream, FRESH_MS, type StreamSnapshot } from './stream'

const T0 = Date.parse('2026-11-26T10:00:00Z')
const message = (id: string, seconds: number): ActivityEvent => ({ id, at: new Date(T0 + seconds * 1000).toISOString(), kind: 'message', from: 'lead', to: 'backend', summary: '' })

/** A node whose answers come from a list of events by time: strictly after `since`, as the real one's. */
function nodeWith(all: ActivityEvent[], options: { limit?: number } = {}) {
  const asked: string[] = []
  return {
    asked,
    add: (event: ActivityEvent) => all.push(event),
    read: (org: string, team: string, since: string): Promise<Activity> => {
      asked.push(since)
      const after = all.filter((event) => Date.parse(event.at) > Date.parse(since)).toSorted((a, b) => Date.parse(a.at) - Date.parse(b.at))
      const events = options.limit === undefined ? after : after.slice(0, options.limit)
      return Promise.resolve({ org, team, generatedAt: new Date(T0 + 60_000).toISOString(), events, truncated: after.length > events.length })
    },
  }
}

const streamOf = (node: { read: (org: string, team: string, since: string) => Promise<Activity> }, now = () => T0) => new ActivityStream({ read: node.read, teams: [{ org: 'acme', team: 'app' }], now })

describe('an activity stream', () => {
  it('reads the first hour at first and takes nothing of it for news', async () => {
    const node = nodeWith([message('m-1', -30), message('m-2', -10)])
    const stream = streamOf(node)
    await stream.poll()
    expect(node.asked[0]).toBe(new Date(T0 - 3_600_000).toISOString())
    expect(stream.current.events.map((one) => one.event.id)).toEqual(['m-1', 'm-2'])
    expect(stream.current.fresh).toEqual([])
  })

  it('then reads from a second before the last event, shows each event once, and calls the new ones fresh', async () => {
    const node = nodeWith([message('m-1', -10)])
    let clock = T0
    const stream = streamOf(node, () => clock)
    await stream.poll()
    node.add(message('m-2', -10)) // the same time as the last one: a read strictly after it would miss it without the overlap
    node.add(message('m-3', 5))
    clock += 10_000
    await stream.poll()
    expect(node.asked[1]).toBe(new Date(T0 - 11_000).toISOString())
    expect(stream.current.events.map((one) => one.event.id)).toEqual(['m-1', 'm-2', 'm-3'])
    expect(stream.current.fresh.map((one) => one.event.id)).toEqual(['m-2', 'm-3'])
    // A few seconds on, nothing is news any more, and nothing is shown twice.
    clock += FRESH_MS + 1
    await stream.poll()
    expect(stream.current.events).toHaveLength(3)
    expect(stream.current.fresh).toEqual([])
  })

  it('keeps what two teams that answered together have just sent, so that a screen that draws once for both sees both', async () => {
    const [first, second] = [nodeWith([]), nodeWith([])]
    let clock = T0
    const stream = new ActivityStream({
      read: (org, team, since) => (team === 'app' ? first : second).read(org, team, since),
      teams: [{ org: 'acme', team: 'app' }, { org: 'acme', team: 'site' }],
      now: () => clock,
    })
    await stream.poll()
    first.add(message('m-a', 70))
    second.add(message('m-b', 71))
    clock += 10_000
    const heard: StreamSnapshot[] = []
    stream.subscribe((snapshot) => heard.push(snapshot))
    await stream.poll()
    // Whatever the screen saw of the two notifications, the last one has both.
    expect(heard.at(-1)?.fresh.map((one) => one.event.id).toSorted()).toEqual(['m-a', 'm-b'])
  })

  it('asks again from where it stopped when the node cut the answer', async () => {
    const node = nodeWith([message('m-1', -50), message('m-2', -40), message('m-3', -30), message('m-4', -20)], { limit: 2 })
    const stream = streamOf(node)
    await stream.poll()
    expect(stream.current.events.map((one) => one.event.id)).toEqual(['m-1', 'm-2', 'm-3', 'm-4'])
  })

  it('names the team that could not be read, keeps what it has, and recovers', async () => {
    let isDown = false
    const node = nodeWith([message('m-1', -10)])
    const stream = streamOf({ read: (org, team, since) => (isDown ? Promise.reject(new Error('503')) : node.read(org, team, since)) })
    await stream.poll()
    isDown = true
    await stream.poll()
    expect(stream.current.failing).toEqual(['acme/app'])
    expect(stream.current.events).toHaveLength(1)
    isDown = false
    node.add(message('m-2', 5))
    await stream.poll()
    expect(stream.current.failing).toEqual([])
    expect(stream.current.fresh.map((one) => one.event.id)).toEqual(['m-2'])
  })

  it('keeps only the most recent events', async () => {
    const node = nodeWith(Array.from({ length: 30 }, (_, index) => message(`m-${index}`, -300 + index)))
    const stream = new ActivityStream({ read: node.read, teams: [{ org: 'acme', team: 'app' }], now: () => T0, keep: 10 })
    await stream.poll()
    expect(stream.current.events.map((one) => one.event.id)).toEqual(Array.from({ length: 10 }, (_, index) => `m-${20 + index}`))
  })

  it('polls on its interval, tells its listeners, and stops', async () => {
    vi.useFakeTimers()
    try {
      const node = nodeWith([])
      const stream = new ActivityStream({ read: node.read, teams: [{ org: 'acme', team: 'app' }], now: () => T0, intervalMs: 10_000 })
      const heard: StreamSnapshot[] = []
      stream.subscribe((snapshot) => heard.push(snapshot))
      stream.start()
      await vi.advanceTimersByTimeAsync(0)
      expect(heard).toHaveLength(1)
      await vi.advanceTimersByTimeAsync(10_000)
      expect(heard).toHaveLength(2)
      stream.stop()
      await vi.advanceTimersByTimeAsync(30_000)
      expect(heard).toHaveLength(2)
    } finally {
      vi.useRealTimers()
    }
  })

  it('says a team whose node does not report activity is not late, and asks it again only now and then', async () => {
    let calls = 0
    const stream = new ActivityStream({ read: () => (++calls, Promise.reject(new ApiError('The activity is not available.', 404))), teams: [{ org: 'acme', team: 'app' }], now: () => T0 })
    await stream.poll()
    expect(stream.current).toMatchObject({ failing: [], unreported: ['acme/app'] })
    for (let round = 0; round < 4; round += 1) await stream.poll()
    expect(calls).toBe(1)
    await stream.poll()
    expect(calls).toBe(2)
  })

  it('counts a read that never answers as a failure of that team, after the time it may take, and goes on', async () => {
    vi.useFakeTimers()
    try {
      let hangs = true
      const node = nodeWith([message('m-1', -10)])
      const stream = new ActivityStream({
        read: (org, team, since, signal) => (hangs ? new Promise((_, reject) => signal.addEventListener('abort', () => reject(signal.reason))) : node.read(org, team, since)),
        teams: [{ org: 'acme', team: 'app' }],
        now: () => T0,
        intervalMs: 10_000,
        timeoutMs: 8_000,
      })
      stream.start()
      await vi.advanceTimersByTimeAsync(8_000)
      expect(stream.current.failing).toEqual(['acme/app'])
      hangs = false
      await vi.advanceTimersByTimeAsync(10_000)
      expect(stream.current.failing).toEqual([])
      expect(stream.current.events.map((one) => one.event.id)).toEqual(['m-1'])
      stream.stop()
    } finally {
      vi.useRealTimers()
    }
  })

  it('does not read while the page is hidden, reads at once when it is shown, and does not take what piled up for news', async () => {
    vi.useFakeTimers()
    let isHidden = true
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => isHidden })
    try {
      let clock = T0
      const node = nodeWith([message('m-1', -10)])
      const stream = new ActivityStream({ read: node.read, teams: [{ org: 'acme', team: 'app' }], now: () => clock, intervalMs: 10_000 })
      stream.start()
      await vi.advanceTimersByTimeAsync(60_000)
      expect(node.asked).toHaveLength(0)
      isHidden = false
      document.dispatchEvent(new Event('visibilitychange'))
      await vi.advanceTimersByTimeAsync(0)
      expect(node.asked).toHaveLength(1)
      // The page is hidden again for ten minutes, in which things happen; on return they are the past, not news.
      isHidden = true
      document.dispatchEvent(new Event('visibilitychange'))
      clock += 10 * 60_000
      node.add(message('m-2', 300))
      await vi.advanceTimersByTimeAsync(60_000)
      expect(node.asked).toHaveLength(1)
      isHidden = false
      document.dispatchEvent(new Event('visibilitychange'))
      await vi.advanceTimersByTimeAsync(0)
      expect(stream.current.events.map((one) => one.event.id)).toEqual(['m-1', 'm-2'])
      expect(stream.current.fresh).toEqual([])
      // And the round after that is news again.
      clock += 10_000
      node.add(message('m-3', 400))
      await vi.advanceTimersByTimeAsync(10_000)
      expect(stream.current.fresh.map((one) => one.event.id)).toEqual(['m-3'])
      stream.stop()
    } finally {
      Reflect.deleteProperty(document, 'hidden')
      vi.useRealTimers()
    }
  })

  it('remembers the ids of the events it keeps, so that pruning does not show one twice', async () => {
    const events = Array.from({ length: 12 }, (_, index) => message(`m-${index}`, -100 + index))
    const node = nodeWith(events)
    const stream = new ActivityStream({ read: node.read, teams: [{ org: 'acme', team: 'app' }], now: () => T0, keep: 3 })
    await stream.poll()
    await stream.poll()
    expect(stream.current.events.map((one) => one.event.id)).toEqual(['m-9', 'm-10', 'm-11'])
    node.add(message('m-12', 5))
    await stream.poll()
    expect(stream.current.events.map((one) => one.event.id)).toEqual(['m-10', 'm-11', 'm-12'])
  })

  it('reads each team on its own, so that one that is slow to answer holds back no other', async () => {
    vi.useFakeTimers()
    try {
      const fast = nodeWith([message('m-1', -10)])
      let release: (() => void) | undefined
      const stream = new ActivityStream({
        read: (org, team, since, signal) =>
          team === 'slow'
            ? new Promise((_, reject) => {
                release = () => reject(new Error('late'))
                signal.addEventListener('abort', () => reject(signal.reason))
              })
            : fast.read(org, team, since),
        teams: [{ org: 'acme', team: 'quick' }, { org: 'acme', team: 'slow' }],
        now: () => T0,
        intervalMs: 10_000,
        timeoutMs: 30_000,
      })
      stream.start()
      await vi.advanceTimersByTimeAsync(0)
      expect(stream.current.events.map((one) => one.event.id)).toEqual(['m-1'])
      fast.add(message('m-2', 5))
      await vi.advanceTimersByTimeAsync(10_000)
      // The slow team has not answered in 10 seconds, and the quick one has gone on regardless.
      expect(stream.current.events.map((one) => one.event.id)).toEqual(['m-1', 'm-2'])
      expect(stream.current.fresh.map((one) => one.event.id)).toEqual(['m-2'])
      release?.()
      stream.stop()
    } finally {
      vi.useRealTimers()
    }
  })

  it('says which teams have not been read yet, and which could not be read at all', async () => {
    const answers = new Map<string, 'ok' | 'fails' | 'unreported'>([['app', 'ok'], ['site', 'fails'], ['lab', 'unreported']])
    const read = (org: string, team: string): Promise<Activity> => {
      const answer = answers.get(team)
      if (answer === 'fails') return Promise.reject(new ApiError('The node did not answer.', 503))
      if (answer === 'unreported') return Promise.reject(new ApiError('Not reported.', 404))
      return Promise.resolve({ org, team, generatedAt: new Date(T0).toISOString(), events: [], truncated: false })
    }
    const stream = new ActivityStream({ read, teams: ['app', 'site', 'lab'].map((team) => ({ org: 'acme', team })), now: () => T0 })
    expect(stream.current.pending).toEqual(['acme/app', 'acme/lab', 'acme/site'])
    await stream.poll()
    expect(stream.current.pending, 'each has been asked once, whatever it answered').toEqual([])
    expect(stream.current.neverRead, 'only the one that failed: a team that does not report has answered').toEqual(['acme/site'])
    answers.set('site', 'ok')
    await stream.poll()
    expect(stream.current.neverRead).toEqual([])
    answers.set('site', 'fails')
    await stream.poll()
    expect(stream.current.failing).toEqual(['acme/site'])
    expect(stream.current.neverRead, 'one that has been read once and then fails is late, not unread').toEqual([])
  })
})

