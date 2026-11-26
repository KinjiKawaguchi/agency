import type { DeviceChangeOutcome, Me, RootChangeRef } from '../api/clientApi'

/**
 * A held change as the page reads it from the node's answer. The generated type is what this client is built for; what comes
 * over the wire is checked, since a node newer than the page may send a kind of change it has no words for, or leave a field out.
 */
export type HeldChange = Omit<RootChangeRef, 'kind' | 'member' | 'what'> & { kind?: string; member?: string; what?: string }

const text = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)

/** The held changes the node says wait, oldest first as the node gives them; an entry that is not one is left out. */
export function rootChangesOf(me: Me): HeldChange[] {
  const held: unknown = me.rootChanges
  if (!Array.isArray(held)) return []
  return held.flatMap((entry: unknown) => {
    if (typeof entry !== 'object' || entry === null) return []
    const { id, org, what, receivedAt, kind, member, label, device } = entry as Record<string, unknown>
    const [one, orgId, at] = [text(id), text(org), text(receivedAt)]
    if (one === undefined || orgId === undefined || at === undefined) return []
    const fields = { what: text(what), kind: text(kind), member: text(member), label: text(label), device: text(device) }
    return [{ id: one, org: orgId, receivedAt: at, ...Object.fromEntries(Object.entries(fields).filter(([, value]) => value !== undefined)) }]
  })
}

/** The outcomes of a device change this client knows: a new one in the generated type is a compile error here, to be worded. */
const KNOWN_OUTCOMES: Record<DeviceChangeOutcome, true> = { applied: true, delivered: true, pending: true, queued: true }

/** What the node answered to a device change, as far as this client says it: held for the root's owner, or an outcome it does not know. */
export type ChangeAnswer = { status: 'pending' } | { status: 'unknown'; message: string } | { status: 'done' }

/**
 * `pending` is the contract, and the page says it in its own words. The node's `message` is used only for an outcome this
 * client does not know, so that it is not shown as a success; an unknown outcome with no message is taken as done.
 */
export function answerOf(result: unknown): ChangeAnswer {
  const answer = typeof result === 'object' && result !== null ? (result as { outcome?: unknown; message?: unknown }) : {}
  if (answer.outcome === 'pending') return { status: 'pending' }
  const message = text(answer.message)
  if (typeof answer.outcome === 'string' && !Object.hasOwn(KNOWN_OUTCOMES, answer.outcome) && message !== undefined) return { status: 'unknown', message }
  return { status: 'done' }
}
