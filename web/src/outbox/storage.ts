// The decisions that are kept in the browser until the node has taken them, so that a reload or a closed tab loses none. What is read
// back is checked: an entry that is not one is left out, never repaired.
import type { OutboxEntry, Phase } from './entry'

export const OUTBOX_STORAGE_KEY = 'agency.outbox'

const PHASES: readonly Phase[] = ['sending', 'retrying', 'failed']

const isText = (value: unknown): value is string => typeof value === 'string' && value !== ''

function isEntry(value: unknown): value is OutboxEntry {
  if (typeof value !== 'object' || value === null) return false
  const entry = value as Record<string, unknown>
  const envelope = entry.envelope as { body?: { id?: unknown }; signature?: unknown } | undefined
  const draft = entry.draft as { text?: unknown; choice?: unknown } | undefined
  return (
    [entry.id, entry.org, entry.team, entry.itemKey, entry.createdAt].every(isText) &&
    typeof entry.itemTitle === 'string' &&
    typeof entry.text === 'string' &&
    typeof entry.attempts === 'number' &&
    PHASES.includes(entry.phase as Phase) &&
    typeof envelope === 'object' && envelope !== null && envelope.body?.id === entry.id && envelope.signature !== undefined &&
    typeof draft === 'object' && draft !== null && typeof draft.text === 'string' && (draft.choice === null || typeof draft.choice === 'string')
  )
}

/** The kept decisions, oldest first; none when there is nothing, or storage cannot be read. */
export function loadOutbox(storage: Pick<Storage, 'getItem'> | undefined = safeStorage()): OutboxEntry[] {
  try {
    const stored: unknown = JSON.parse(storage?.getItem(OUTBOX_STORAGE_KEY) ?? '[]')
    return Array.isArray(stored) ? stored.filter(isEntry) : []
  } catch {
    return []
  }
}

/** Keeps the decisions; a storage that is full or refused is not an error of the desk, which still holds them in memory. */
export function saveOutbox(entries: readonly OutboxEntry[], storage: Pick<Storage, 'setItem' | 'removeItem'> | undefined = safeStorage()): void {
  try {
    if (entries.length === 0) storage?.removeItem(OUTBOX_STORAGE_KEY)
    else storage?.setItem(OUTBOX_STORAGE_KEY, JSON.stringify(entries))
  } catch {
    // Kept in memory only, as far as this page lives.
  }
}

function safeStorage(): Storage | undefined {
  try {
    return window.localStorage
  } catch {
    return undefined
  }
}
