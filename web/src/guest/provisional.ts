// How the client learns that the person is a member hosted by the node (#201, design on #196 section 4). The node says it
// as `Me.guest = { member, org }`, present only for a guest (AgencyCore's answer on #201); it is not in the generated `Me`
// yet, so the shape is kept in this one module, and taking the generated type in is one import and the body of `guestOf`.
import type { Me } from '../api/clientApi'

/** A `Me` that may say the viewer is a hosted member (a guest of the node), which member they are, and in which org. */
export type MeWithGuest = Me & { guest?: { member: string; org: string } }

export interface Guest {
  /** Which member they are, when the node says so in the shape this client expects. */
  member?: string
  /** The org they have their seat in, when it says so. */
  org?: string
}

/**
 * The guest the node says the viewer is; nothing for anyone else. Any `guest` that is not null counts, whatever its shape: if
 * the node says it in another shape, the viewer must still get a guest's pages, not the owner's menu and refusals.
 */
export function guestOf(me: Me): Guest | undefined {
  const guest: unknown = (me as MeWithGuest).guest
  if (guest === undefined || guest === null) return undefined
  const { member, org } = (typeof guest === 'object' ? guest : {}) as { member?: unknown; org?: unknown }
  return { ...(typeof member === 'string' && member !== '' ? { member } : {}), ...(typeof org === 'string' && org !== '' ? { org } : {}) }
}
