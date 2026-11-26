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
  /** The events that arrived in the latest read of an already running feed: the ones to make a stir about. The first read is the past. */
  fresh: readonly TaggedEvent[]
  /** The teams whose latest read failed. */
  failing: readonly string[]
  /** The teams whose node does not report activity (404): their feed is not late, it does not exist yet. */
  unreported: readonly string[]
}

interface Options {
  read: (org: string, team: string, since: string) => Promise<Activity>
  teams: readonly Team[]
  now?: () => number
  /** How often every team is read. */
  intervalMs?: number
  /** How far back the first read goes. */
  firstWindowMs?: number
  /** How many events are kept. */
  keep?: number
}

const key = (team: Team): string => `${team.org}/${team.team}`
/** The node's `since` is strictly after, and two events may share a time, so the next read starts a second early and the ids dedupe. */
const OVERLAP_MS = 1_000
/** A read the node cut at its limit is asked again from where it stopped, at most this many times a poll. */
const MAX_PAGES = 5
/** A team whose node does not report activity is asked again every this many rounds. */
const UNREPORTED_EVERY = 6

/**
 * A feed that keeps itself up to date: it reads every team's activity on an interval, from the last event seen (a
 * second early), keeps each event once by its id, and tells its listeners what is new. Snapshots are never changed.
 */
export class ActivityStream {
  private readonly options: Required<Options>
  private snapshot: StreamSnapshot = { events: [], fresh: [], failing: [], unreported: [] }
  private readonly listeners = new Set<(snapshot: StreamSnapshot) => void>()
  private readonly seen = new Map<string, Set<string>>()
  private readonly cursors = new Map<string, number>()
  private readonly failing = new Set<string>()
  private readonly unreported = new Set<string>()
  private round = 0
  private timer: ReturnType<typeof setInterval> | undefined
  private isFirstRead = true
  private isReading = false
  private isStopped = false

  constructor(options: Options) {
    this.options = { now: Date.now, intervalMs: 10_000, firstWindowMs: 3_600_000, keep: 1_000, ...options }
  }

  get current(): StreamSnapshot {
    return this.snapshot
  }

  subscribe(listener: (snapshot: StreamSnapshot) => void): () => void {
    this.listeners.add(listener)
    return () => void this.listeners.delete(listener)
  }

  start(): void {
    void this.poll()
    this.timer = setInterval(() => void this.poll(), this.options.intervalMs)
  }

  stop(): void {
    this.isStopped = true
    if (this.timer !== undefined) clearInterval(this.timer)
    this.listeners.clear()
  }

  /** One round: every team is read, and the listeners hear once. A round that starts while another runs is skipped. */
  async poll(): Promise<void> {
    if (this.isReading) return
    this.isReading = true
    try {
      this.round += 1
      // A team whose node does not report activity is asked again once in a while, not every time.
      const asked = this.options.teams.filter((team) => !this.unreported.has(key(team)) || this.round % UNREPORTED_EVERY === 0)
      const results = await Promise.all(asked.map((team) => this.readTeam(team)))
      if (this.isStopped) return
      const fresh = results.flat()
      const events = [...this.snapshot.events, ...fresh].toSorted((a, b) => Date.parse(a.event.at) - Date.parse(b.event.at)).slice(-this.options.keep)
      this.snapshot = { events, fresh: this.isFirstRead ? [] : fresh, failing: [...this.failing].toSorted(), unreported: [...this.unreported].toSorted() }
      this.isFirstRead = false
      for (const listener of this.listeners) listener(this.snapshot)
    } finally {
      this.isReading = false
    }
  }

  private async readTeam(team: Team): Promise<TaggedEvent[]> {
    const name = key(team)
    const seen = this.seen.get(name) ?? new Set<string>()
    this.seen.set(name, seen)
    const added: TaggedEvent[] = []
    try {
      for (let page = 0; page < MAX_PAGES; page += 1) {
        const from = this.cursors.get(name) === undefined ? this.options.now() - this.options.firstWindowMs : (this.cursors.get(name) as number) - OVERLAP_MS
        const activity = await this.options.read(team.org, team.team, new Date(from).toISOString())
        for (const event of activity.events) {
          if (seen.has(event.id)) continue
          seen.add(event.id)
          added.push({ org: team.org, team: team.team, event })
        }
        const last = activity.events.at(-1)
        this.cursors.set(name, last === undefined ? Date.parse(activity.generatedAt) : Date.parse(last.at))
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
}
