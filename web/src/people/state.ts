import type { PersonStatus } from '../api/teamview'

/** What the viewer needs to know about a person at a glance. */
export type Presence = 'needs-you' | 'working' | 'waiting' | 'idle' | 'away'

/** The team runtime's own name for the team's owner, the person the viewer answers for. */
export const OWNER_NAME = 'owner'

/**
 * `needs-you`: the person can only continue after the owner acts.
 * `waiting`: they wait on someone else. `unknown` counts as idle; nothing is shown for it.
 */
export function presenceOf(person: PersonStatus): Presence {
  if (person.waitingOn?.kind === 'person' && person.waitingOn.person === OWNER_NAME) return 'needs-you'
  if (person.state === 'away') return 'away'
  if (person.waitingOn !== undefined) return 'waiting'
  return person.state === 'working' ? 'working' : 'idle'
}
