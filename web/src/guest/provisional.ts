// How the client learns that the person is a member hosted by the node (#201, design on #196 section 4). The node's `Me`
// does not carry this yet: the field below is the shape this client assumes, kept in this one module so that taking the
// generated type in is one import and the body of `guestOf`.
import type { Me } from '../api/clientApi'

/** A `Me` that may say the viewer is a hosted member (a guest of the node) and which member they are. */
export type MeWithGuest = Me & { guest?: { member: string } }

export interface Guest {
  /** Which member they are, when the node says so in the shape this client expects. */
  member?: string
}

/**
 * The guest the node says the viewer is; nothing for anyone else. Any `guest` that is not null counts, whatever its shape: if
 * the node says it in another shape, the viewer must still get a guest's pages, not the owner's menu and refusals.
 */
export function guestOf(me: Me): Guest | undefined {
  const guest: unknown = (me as MeWithGuest).guest
  if (guest === undefined || guest === null) return undefined
  const member = typeof guest === 'object' ? (guest as { member?: unknown }).member : undefined
  return typeof member === 'string' && member !== '' ? { member } : {}
}
