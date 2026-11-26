// The changes others asked the root key to sign, which wait for its owner (`Me.rootChanges`, and the outcome `pending` of a
// device change; the node side is agency#381). Until they are generated into `bindings/`, the shapes are written down here
// and nowhere else: taking the generated types in is the replacement of this file by their imports.
import type { Me } from '../api/clientApi'

/** A held change, as the owner's desk shows it. */
export interface RootChangeRef {
  id: string
  org: string
  /** What it changes, in the node's words: "add member carol (Carol)", "add device iPhone for bob". */
  what: string
  receivedAt: string
}

export type MeWithRootChanges = Me & { rootChanges?: RootChangeRef[] }

/** What a device change answers when the root key's node holds it for its owner's `agency root-sign`. */
export interface PendingChange {
  outcome: 'pending'
  message?: string
}

const text = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)

/** The held changes the node says wait, oldest first as the node gives them; an entry that is not one is left out. */
export function rootChangesOf(me: Me): RootChangeRef[] {
  const held: unknown = (me as MeWithRootChanges).rootChanges
  if (!Array.isArray(held)) return []
  return held.flatMap((entry: unknown) => {
    if (typeof entry !== 'object' || entry === null) return []
    const { id, org, what, receivedAt } = entry as Record<string, unknown>
    const [one, orgId, words, at] = [text(id), text(org), text(what), text(receivedAt)]
    return one === undefined || orgId === undefined || words === undefined || at === undefined ? [] : [{ id: one, org: orgId, what: words, receivedAt: at }]
  })
}

/** The message of a device change the root's node holds, or nothing for any other answer. */
export function pendingOf(result: unknown): PendingChange | undefined {
  if (typeof result !== 'object' || result === null || (result as { outcome?: unknown }).outcome !== 'pending') return undefined
  const message = text((result as { message?: unknown }).message)
  return { outcome: 'pending', ...(message === undefined ? {} : { message }) }
}
