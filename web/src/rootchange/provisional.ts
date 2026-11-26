// The changes others asked the root key to sign, which wait for its owner (`Me.rootChanges`, and the outcome `pending` of a
// device change; the node side is agency#381). Until they are generated into `bindings/`, the shapes are written down here
// and nowhere else: taking the generated types in is the replacement of this file by their imports.
import type { Me } from '../api/clientApi'

/** A held change, as the owner's desk shows it. */
export interface RootChangeRef {
  id: string
  org: string
  /** What it changes, in the node's English words: "add member carol (Carol)". Said only for a kind this client does not know. */
  what: string
  receivedAt: string
  /** The kind of change, and the fields it is worded from (assumed shape, asked of the node): `addMember` (member, label), `addDevice` (member, label), `removeDevice` (member, device). */
  kind?: string
  /** The new member for `addMember`; the owner of the device for the other two. */
  member?: string
  /** The member's label for `addMember`; the device's label for `addDevice`. */
  label?: string
  /** The device's id for `removeDevice`. */
  device?: string
}

export type MeWithRootChanges = Me & { rootChanges?: RootChangeRef[] }

/** The outcomes of a device change this client knows (`DeviceChangeOutcome`). */
export const KNOWN_OUTCOMES: readonly string[] = ['applied', 'delivered', 'pending', 'queued']

const text = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)

/** The held changes the node says wait, oldest first as the node gives them; an entry that is not one is left out. */
export function rootChangesOf(me: Me): RootChangeRef[] {
  const held: unknown = (me as MeWithRootChanges).rootChanges
  if (!Array.isArray(held)) return []
  return held.flatMap((entry: unknown) => {
    if (typeof entry !== 'object' || entry === null) return []
    const { id, org, what, receivedAt } = entry as Record<string, unknown>
    const [one, orgId, words, at] = [text(id), text(org), text(what), text(receivedAt)]
    if (one === undefined || orgId === undefined || words === undefined || at === undefined) return []
    const { kind, member, label, device } = entry as Record<string, unknown>
    const [kindText, memberText, labelText, deviceText] = [text(kind), text(member), text(label), text(device)]
    return [
      {
        id: one,
        org: orgId,
        what: words,
        receivedAt: at,
        ...(kindText === undefined ? {} : { kind: kindText }),
        ...(memberText === undefined ? {} : { member: memberText }),
        ...(labelText === undefined ? {} : { label: labelText }),
        ...(deviceText === undefined ? {} : { device: deviceText }),
      },
    ]
  })
}

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
  if (typeof answer.outcome === 'string' && !KNOWN_OUTCOMES.includes(answer.outcome) && message !== undefined) return { status: 'unknown', message }
  return { status: 'done' }
}
