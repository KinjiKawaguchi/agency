// A recovery of an org that was started and waits, as the node's `GET /api/me` gives it (docs/protocol.md, "Client API"; the node
// side is agency#360). Until it is generated into `bindings/`, the shape is written down here and nowhere else: taking the
// generated type in is the replacement of this file by its import, and `recoveriesOf` below.
import type { Me, OrgRef } from '../api/clientApi'

/** What a started recovery will do, and when this node takes its finish. */
export interface StartedRecovery {
  /** When it takes effect on this node; absent when the node kept no record of when it took the start. */
  takesEffectAt?: string
  /** The fingerprint of the root key it moves the org to; absent when it keeps the root key. */
  newRootKey?: string
  /** Whether it replaces the recovery key too; the node leaves it out, never `false`. */
  replacesRecoveryKey?: boolean
}

export type OrgWithRecovery = OrgRef & { recovery?: StartedRecovery }

const text = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)

/** One org's recovery as the page shows it, with the org's label and the id the command takes. */
export interface Recovery {
  org: { id: string; label: string }
  /** Whether this node holds the org's root key: only its owner can cancel the recovery, and the others are only told of it. */
  isRoot: boolean
  takesEffectAt?: string
  newRootKey?: string
  replacesRecoveryKey: boolean
}

/** The recoveries the node says wait, one for each org that has one, in the node's order. An org with none gives none. */
export function recoveriesOf(me: Me): Recovery[] {
  return (me.orgs as OrgWithRecovery[]).flatMap((org) => {
    const started: unknown = org.recovery
    if (typeof started !== 'object' || started === null) return []
    const { takesEffectAt, newRootKey, replacesRecoveryKey } = started as Record<string, unknown>
    const at = text(takesEffectAt)
    const key = text(newRootKey)
    return [{ org: { id: org.id, label: org.label }, isRoot: org.isRoot === true, ...(at === undefined ? {} : { takesEffectAt: at }), ...(key === undefined ? {} : { newRootKey: key }), replacesRecoveryKey: replacesRecoveryKey === true }]
  })
}
