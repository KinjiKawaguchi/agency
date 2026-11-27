import { useQuery, useQueryClient } from '@tanstack/react-query'
import { createContext, useCallback, useContext, useEffect, useMemo, useReducer, useRef, useState, type ReactNode } from 'react'
import { ApiError } from '../api/client'
import { useApi } from '../api/context'
import { DECISIONS_KEY, decisionsQuery, meQuery, TEAM_KEY } from '../api/queries'
import { SignInExpiredError } from '../api/session'
import { afterFailure, classify, dueEntries, nextDueAt, sendAgain, type OutboxEntry } from './entry'
import { loadOutbox, OUTBOX_STORAGE_KEY, saveOutbox } from './storage'

/** How many decisions are on their way at once. */
const MOST_IN_FLIGHT = 3
/** While a decision is kept or queued at the node, the log is read this often, to see it end. */
const WATCH_MS = 3_000

export interface Outbox {
  /** Every decision kept until the node has it, oldest first. */
  entries: readonly OutboxEntry[]
  /** The requests decided and not yet done with, as the desk keys them: kept here, or queued at the node. They stay off the pile. */
  hiddenKeys: ReadonlySet<string>
  enqueue: (entry: OutboxEntry) => void
  /** Sends a kept decision again, the same envelope, at once. */
  sendAgain: (id: string) => void
  /** Gives up a decision that has not been taken: the request comes back on the pile, and the desk is told so that it can give the draft back. */
  discard: (id: string) => void
  onDiscard: (listener: (entry: OutboxEntry) => void) => () => void
}

const NONE: Outbox = { entries: [], hiddenKeys: new Set(), enqueue: () => undefined, sendAgain: () => undefined, discard: () => undefined, onDiscard: () => () => undefined }

const OutboxContext = createContext<Outbox>(NONE)

/** The decisions in flight, and what is kept off the pile. Outside a provider there are none (a page that never decides). */
export const useOutbox = (): Outbox => useContext(OutboxContext)

const failureOf = (error: unknown): Parameters<typeof classify>[0] => {
  if (error instanceof SignInExpiredError) return { message: error.message, isSignIn: true }
  if (error instanceof ApiError) return { status: error.status, message: error.message, ...(error.retryAfterSeconds === undefined ? {} : { retryAfterSeconds: error.retryAfterSeconds }) }
  return { message: error instanceof Error ? error.message : String(error), isNetwork: true }
}

/**
 * Keeps the decisions the owner made until the node has taken them, and sends them: one in flight at a time for each, three at once,
 * tried again after 5 seconds, 15 seconds, a minute, 5 minutes and every 5 minutes, or at once when the owner says so; stopped for a refusal
 * that will not change; paused while the sign-in is gone. They are kept in the browser (a reload resumes them) and never dropped before
 * the node says it took one: every 2xx is "taken". A request stays off the pile while its decision is kept here or queued at the node.
 */
export function OutboxProvider({ children, now = Date.now }: { children: ReactNode; now?: () => number }) {
  const api = useApi()
  const queryClient = useQueryClient()
  const [entries, setEntries] = useState<readonly OutboxEntry[]>(() => loadOutbox())
  const [isPaused, setIsPaused] = useState(false)
  /** The requests of decisions the node has taken, while what it says of them is being read. */
  const [settling, setSettling] = useState<ReadonlySet<string>>(() => new Set())
  // A counter that wakes the effect that starts what is due; never drawn.
  const [tick, wake] = useReducer((count: number) => count + 1, 1)
  const inFlight = useRef(new Set<string>())
  const listeners = useRef(new Set<(entry: OutboxEntry) => void>())
  // What is kept is written at the moment it changes, before the next render and before the desk moves on: a browser killed an instant after the
  // owner decided has the decision. The ref is the truth the changes are made on; the state is what is drawn.
  const entriesRef = useRef(entries)
  const commit = useCallback((next: readonly OutboxEntry[]) => {
    entriesRef.current = next
    saveOutbox(next)
    setEntries(next)
  }, [])
  // Another tab of the same browser keeps and ends decisions too: what it wrote is the truth.
  useEffect(() => {
    const onStorage = (event: StorageEvent): void => {
      if (event.key !== OUTBOX_STORAGE_KEY && event.key !== null) return
      entriesRef.current = loadOutbox()
      setEntries(entriesRef.current)
    }
    window.addEventListener('storage', onStorage)
    return () => window.removeEventListener('storage', onStorage)
  }, [])
  useEffect(() => api.onSession((state) => setIsPaused(state === 'expired')), [api])

  const update = useCallback(
    (id: string, change: (entry: OutboxEntry) => OutboxEntry | undefined) => {
      commit(
        entriesRef.current.flatMap((entry) => {
          if (entry.id !== id) return [entry]
          const changed = change(entry)
          return changed === undefined ? [] : [changed]
        }),
      )
    },
    [commit],
  )

  const attempt = useCallback(
    async (entry: OutboxEntry) => {
      inFlight.current.add(entry.id)
      try {
        await api.decideSigned(entry.org, entry.team, { envelope: entry.envelope, ...(entry.title === undefined ? {} : { title: entry.title }), ...(entry.choiceLabel === undefined ? {} : { choiceLabel: entry.choiceLabel }) })
        // The node has it: the entry is done at once. The request stays off the pile until what the node says of it has been read, so that it does
        // not come back for the moment between (a slow team node makes that moment long).
        update(entry.id, () => undefined)
        setSettling((keys) => new Set([...keys, entry.itemKey]))
        void Promise.all([queryClient.invalidateQueries({ queryKey: DECISIONS_KEY }), queryClient.invalidateQueries({ queryKey: TEAM_KEY })]).finally(() => setSettling((keys) => new Set([...keys].filter((key) => key !== entry.itemKey))))
      } catch (error) {
        update(entry.id, (current) => afterFailure(current, classify(failureOf(error)), now()))
      } finally {
        inFlight.current.delete(entry.id)
        wake()
      }
    },
    [api, now, queryClient, update],
  )

  // Starts what is due, and wakes at the time the next waiting one is.
  useEffect(() => {
    if (isPaused) return undefined
    const room = MOST_IN_FLIGHT - inFlight.current.size
    for (const entry of dueEntries(entries, inFlight.current, now()).slice(0, Math.max(0, room))) void attempt(entry)
    const next = nextDueAt(entries)
    if (next === undefined) return undefined
    const timer = window.setTimeout(() => wake(), Math.max(0, next - now()))
    return () => window.clearTimeout(timer)
  }, [attempt, entries, isPaused, now, tick])

  const hasPending = entries.length > 0
  // The log is read where the viewer is known (a person on the claim page has no seat yet, and the node may refuse it).
  const me = useQuery(meQuery(api))
  const log = useQuery({ ...decisionsQuery(api), enabled: me.data !== undefined, refetchInterval: hasPending ? WATCH_MS : decisionsQuery(api).refetchInterval })
  const queuedKeys = useMemo(
    () => new Set((log.data?.entries ?? [] /* stand-in-ok(optional): before the log has been read nothing is known to be queued; the entries kept here hide their own requests */).flatMap((entry) => (entry.state.kind === 'queued' ? [`${entry.org}:${entry.team}:${entry.target}`] : []))),
    [log.data],
  )
  const hiddenKeys = useMemo(() => new Set([...entries.map((entry) => entry.itemKey), ...settling, ...queuedKeys]), [entries, queuedKeys, settling])

  // A decision that was queued at the node and is not any more has ended: the team's view says what became of its request.
  const wasQueued = useRef<ReadonlySet<string>>(new Set())
  useEffect(() => {
    if ([...wasQueued.current].some((key) => !queuedKeys.has(key))) void queryClient.invalidateQueries({ queryKey: TEAM_KEY })
    wasQueued.current = queuedKeys
  }, [queryClient, queuedKeys])

  const value = useMemo<Outbox>(
    () => ({
      entries,
      hiddenKeys,
      enqueue: (entry) => {
        if (!entriesRef.current.some((one) => one.id === entry.id)) commit([...entriesRef.current, entry])
      },
      sendAgain: (id) => {
        update(id, sendAgain)
        wake()
      },
      discard: (id) => {
        const found = entriesRef.current.find((one) => one.id === id)
        update(id, () => undefined)
        if (found !== undefined) listeners.current.forEach((listener) => listener(found))
      },
      onDiscard: (listener) => {
        listeners.current.add(listener)
        return () => void listeners.current.delete(listener)
      },
    }),
    [commit, entries, hiddenKeys, update],
  )
  return <OutboxContext.Provider value={value}>{children}</OutboxContext.Provider>
}
