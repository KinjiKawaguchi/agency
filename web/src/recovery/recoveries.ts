import type { Me } from '../api/clientApi'

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

/** The recoveries the node says wait, one for each org that has one, in the node's order. An org with none gives none; what is read is checked, not trusted. */
export function recoveriesOf(me: Me): Recovery[] {
  return me.orgs.flatMap((org) => {
    const started: unknown = org.recovery
    if (typeof started !== 'object' || started === null) return []
    const { takesEffectAt, newRootKey, replacesRecoveryKey } = started as Record<string, unknown>
    const at = text(takesEffectAt)
    const key = text(newRootKey)
    return [{ org: { id: org.id, label: org.label }, isRoot: org.isRoot === true, ...(at === undefined ? {} : { takesEffectAt: at }), ...(key === undefined ? {} : { newRootKey: key }), replacesRecoveryKey: replacesRecoveryKey === true }]
  })
}

/** The node's rule for an id (`Id::parse`): lowercase letters, digits, and hyphens, not starting with a hyphen, at most 40 characters. */
export const isId = (value: string): boolean => /^[a-z0-9][a-z0-9-]{0,39}$/.test(value)
