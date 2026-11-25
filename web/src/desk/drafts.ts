/** What the viewer has typed or picked for one item, kept until it is sent. */
export interface Draft {
  text: string
  choice: string | null
}

export const EMPTY_DRAFT: Draft = { text: '', choice: null }

export type Drafts = ReadonlyMap<string, Draft>

export const draftOf = (drafts: Drafts, key: string): Draft => drafts.get(key) ?? EMPTY_DRAFT

export const withDraft = (drafts: Drafts, key: string, change: Partial<Draft>): Drafts =>
  new Map([...drafts, [key, { ...draftOf(drafts, key), ...change }]])

export const withoutDraft = (drafts: Drafts, key: string): Drafts => new Map([...drafts].filter(([one]) => one !== key))

const STORAGE_KEY = 'agency.drafts'

const isDraft = (value: unknown): value is Draft =>
  typeof value === 'object' && value !== null && 'text' in value && typeof value.text === 'string' && 'choice' in value && (value.choice === null || typeof value.choice === 'string')

/**
 * Drafts live in the tab's session storage, so a reload (to sign in again, say) keeps what the viewer wrote.
 * Storage can be missing or full; then drafts simply last until the page closes.
 */
export function loadDrafts(): Drafts {
  try {
    const stored: unknown = JSON.parse(window.sessionStorage.getItem(STORAGE_KEY) ?? '[]')
    return new Map(Array.isArray(stored) ? stored.flatMap((entry: unknown) => (Array.isArray(entry) && typeof entry[0] === 'string' && isDraft(entry[1]) ? [[entry[0], entry[1]] as const] : [])) : [])
  } catch {
    return new Map()
  }
}

export function saveDrafts(drafts: Drafts): void {
  try {
    window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify([...drafts]))
  } catch {
    // Not stored: the drafts stay in memory.
  }
}
