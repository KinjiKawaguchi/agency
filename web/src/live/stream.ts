import type { Activity, ActivityEvent } from '../api/activityApi'
import { ApiError } from '../api/client'

export interface Team {
  org: string
  team: string
}

/** An event with the team it came from. */
export interface TaggedEvent {
  org: string
  team: string
  event: ActivityEvent
}

export interface StreamSnapshot {
  /** Everything kept, oldest first. */
  events: readonly TaggedEvent[]
  /**
   * The events that arrived in the last few seconds of an already running feed: the ones to make a stir about. The
   * first read, and what comes after a long gap, is the past. Two teams that answer together are both here, so that
   * a screen that renders once for both sees both; one that acts on them keeps the ids it has acted on.
   */
  fresh: readonly TaggedEvent[]
  /** The teams whose latest read failed. */
  failing: readonly string[]
  /** The teams whose node does not report activity (404): their feed is not late, it does not exist yet. */
  unreported: readonly string[]
  /** The teams that have not been read yet, once: until then what is counted from the events is not the whole. */
  pending: readonly string[]
}

interface Options {
  /** One read; it must give up when `signal` aborts. */
  read: (org: string, team: string, since: string, signal: AbortSignal) => Promise<Activity>
  teams: readonly Team[]
  now?: () => number
  /** How often every team is read. */
  intervalMs?: number
  /** How far back the first read goes. */
  firstWindowMs?: number
  /** How many events are kept. */
  keep?: number
  /** A read that has not answered in this long is a failure of that team: a read that hangs must not stop the rounds. */
  timeoutMs?: number
}

const key = (team: Team): string => `${team.org}/${team.team}`
/** The node's `since` is strictly after, and two events may share a time, so the next read starts a second early and the ids dedupe. */
const OVERLAP_MS = 1_000
/** A read the node cut at its limit is asked again from where it stopped, at most this many times a poll. */
const MAX_PAGES = 5
/** A team whose node does not report activity is asked again every this many rounds. */
const UNREPORTED_EVERY = 6
/** A gap this many intervals long between two rounds (a tab that was hidden) makes what comes after it the past, not news. */
const GAP_INTERVALS = 6
/** How long an event that has just arrived counts as new. */
export const FRESH_MS = 3_000

/** A promise that fails when the signal aborts, to race a read that may ignore it. */
const aborted = (signal: AbortSignal): Promise<never> =>
  new Promise((_, reject) => {
    if (signal.aborted) reject(signal.reason)
    signal.addEventListener('abort', () => reject(signal.reason), { once: true })
  })

/** What the stream knows of one team. */
interface TeamFeed {
  team: Team
  seen: Set<string>
  cursor: number | undefined
  /** Rounds of this team so far, to ask a team that does not report activity only now and then. */
  rounds: number
  lastRoundAt: number | undefined
  isFirstRead: boolean
  timer: ReturnType<typeof setTimeout> | undefined
  isReading: boolean
}

/**
 * A feed that keeps itself up to date: each team is read on its own, on an interval that starts when its last read
 * ends (so a team that is slow to answer holds back no other), from the last event seen (a second early); each event
 * is kept once by its id, and the listeners are told what is new. Snapshots are never changed.
 */
export class ActivityStream {
  private readonly options: Required<Options>
  private snapshot: StreamSnapshot = { events: [], fresh: [], failing: [], unreported: [], pending: [] }
  private readonly listeners = new Set<(snapshot: StreamSnapshot) => void>()
  private readonly feeds: ReadonlyMap<string, TeamFeed>
  private readonly failing = new Set<string>()
  private readonly unreported = new Set<string>()
  private isStopped = false
  private isPaused = true
  private readonly controllers = new Set<AbortController>()
  /** When each event that counts as new arrived. */
  private arrivals: ReadonlyArray<{ tagged: TaggedEvent; at: number }> = []
  private readonly onVisibility = (): void => {
    if (document.hidden) this.pause()
    else this.resume()
  }

  constructor(options: Options) {
    this.options = { now: Date.now, intervalMs: 10_000, firstWindowMs: 3_600_000, keep: 1_000, timeoutMs: 20_000, ...options }
    this.feeds = new Map(options.teams.map((team) => [key(team), { team, seen: new Set<string>(), cursor: undefined, rounds: 0, lastRoundAt: undefined, isFirstRead: true, timer: undefined, isReading: false }]))
    this.snapshot = { ...this.snapshot, pending: [...this.feeds.keys()].toSorted() }
  }

  get current(): StreamSnapshot {
    return this.snapshot
  }

  subscribe(listener: (snapshot: StreamSnapshot) => void): () => void {
    this.listeners.add(listener)
    return () => void this.listeners.delete(listener)
  }

  /** Starts reading; while the page is hidden it does not, and it reads at once when the page is shown again. */
  start(): void {
    if (typeof document !== 'undefined') document.addEventListener('visibilitychange', this.onVisibility)
    if (typeof document === 'undefined' || !document.hidden) this.resume()
  }

  stop(): void {
    this.isStopped = true
    this.pause()
    if (typeof document !== 'undefined') document.removeEventListener('visibilitychange', this.onVisibility)
    for (const controller of this.controllers) controller.abort(new Error('stopped'))
    this.listeners.clear()
  }

  private pause(): void {
    this.isPaused = true
    for (const feed of this.feeds.values()) {
      if (feed.timer !== undefined) clearTimeout(feed.timer)
      feed.timer = undefined
    }
  }

  private resume(): void {
    if (!this.isPaused || this.isStopped) return
    this.isPaused = false
    for (const feed of this.feeds.values()) void this.run(feed)
  }

  /** One round of every team, at once, and the listeners hear after each. Resolves when all have been read. */
  async poll(): Promise<void> {
    await Promise.all([...this.feeds.values()].map((feed) => this.round(feed)))
  }

  /** One round of a team, and the next one when it is time (unless paused or stopped). */
  private async run(feed: TeamFeed): Promise<void> {
    await this.round(feed)
    if (this.isStopped || this.isPaused) return
    feed.timer = setTimeout(() => void this.run(feed), this.options.intervalMs)
  }

  private async round(feed: TeamFeed): Promise<void> {
    if (feed.isReading) return
    const name = key(feed.team)
    feed.rounds += 1
    // A team whose node does not report activity is asked again every few rounds, not every time.
    if (this.unreported.has(name) && feed.rounds % UNREPORTED_EVERY !== 0) return
    feed.isReading = true
    const startedAt = this.options.now()
    const isAfterGap = feed.lastRoundAt !== undefined && startedAt - feed.lastRoundAt > GAP_INTERVALS * this.options.intervalMs
    feed.lastRoundAt = startedAt
    try {
      const added = await this.readTeam(feed)
      if (this.isStopped) return
      const events = [...this.snapshot.events, ...added].toSorted((a, b) => Date.parse(a.event.at) - Date.parse(b.event.at)).slice(-this.options.keep)
      this.prune(events)
      const now = this.options.now()
      this.arrivals = [...this.arrivals, ...(feed.isFirstRead || isAfterGap ? [] : added.map((tagged) => ({ tagged, at: now })))].filter((one) => now - one.at < FRESH_MS)
      feed.isFirstRead = false
      const pending = [...this.feeds.entries()].filter(([, one]) => one.isFirstRead).map(([name]) => name).toSorted()
      this.snapshot = { events, fresh: this.arrivals.map((one) => one.tagged), failing: [...this.failing].toSorted(), unreported: [...this.unreported].toSorted(), pending }
      for (const listener of this.listeners) listener(this.snapshot)
    } finally {
      feed.isReading = false
    }
  }

  private async readTeam(feed: TeamFeed): Promise<TaggedEvent[]> {
    const { team, seen } = feed
    const name = key(team)
    const added: TaggedEvent[] = []
    try {
      for (let page = 0; page < MAX_PAGES; page += 1) {
        const from = feed.cursor === undefined ? this.options.now() - this.options.firstWindowMs : feed.cursor - OVERLAP_MS
        const activity = await this.readOnce(team, from)
        for (const event of activity.events) {
          if (seen.has(event.id)) continue
          seen.add(event.id)
          added.push({ org: team.org, team: team.team, event })
        }
        const last = activity.events.at(-1)
        feed.cursor = last === undefined ? Date.parse(activity.generatedAt) : Date.parse(last.at)
        if (!activity.truncated) break
      }
      this.failing.delete(name)
      this.unreported.delete(name)
    } catch (error) {
      if (error instanceof ApiError && error.status === 404) {
        this.unreported.add(name)
        this.failing.delete(name)
      } else this.failing.add(name)
    }
    return added
  }

  /** One read, with the time it may take: when it does not answer in time it is aborted and counts as a failure of the team. */
  private async readOnce(team: Team, from: number): Promise<Activity> {
    const controller = new AbortController()
    this.controllers.add(controller)
    const timer = setTimeout(() => controller.abort(new Error('The activity did not answer in time.')), this.options.timeoutMs)
    try {
      return await Promise.race([this.options.read(team.org, team.team, new Date(from).toISOString(), controller.signal), aborted(controller.signal)])
    } finally {
      clearTimeout(timer)
      this.controllers.delete(controller)
    }
  }

  /** The ids that are remembered are the ones of the events that are kept, once there are many more of them than that. */
  private prune(kept: readonly TaggedEvent[]): void {
    const total = [...this.feeds.values()].reduce((sum, feed) => sum + feed.seen.size, 0)
    if (total <= this.options.keep * 2) return
    for (const feed of this.feeds.values()) feed.seen.clear()
    for (const { org, team, event } of kept) this.feeds.get(`${org}/${team}`)?.seen.add(event.id)
  }
}
