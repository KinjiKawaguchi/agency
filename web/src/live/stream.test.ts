import type { Activity, ActivityEvent } from '../api/activityApi'
import { ApiError } from '../api/client'
import { ActivityStream, type StreamSnapshot } from './stream'

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
    const stream = streamOf(node)
    await stream.poll()
    node.add(message('m-2', -10)) // the same time as the last one: a read strictly after it would miss it without the overlap
    node.add(message('m-3', 5))
    await stream.poll()
    expect(node.asked[1]).toBe(new Date(T0 - 11_000).toISOString())
    expect(stream.current.events.map((one) => one.event.id)).toEqual(['m-1', 'm-2', 'm-3'])
    expect(stream.current.fresh.map((one) => one.event.id)).toEqual(['m-2', 'm-3'])
    await stream.poll()
    expect(stream.current.events).toHaveLength(3)
    expect(stream.current.fresh).toEqual([])
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
})
