import type { PersonStatus } from '../api/teamview'

/** What the viewer needs to know about a person at a glance. */
export type Presence = 'needs-you' | 'working' | 'waiting' | 'idle' | 'away'

/**
 * `viewerName` is the runtime name of the person the viewer answers for.
 * `needs-you`: the person can only continue after the owner acts.
 * `waiting`: they wait on someone else. `unknown` counts as idle; nothing is shown for it.
 */
export function presenceOf(person: PersonStatus, viewerName: string): Presence {
  if (person.waitingOn?.kind === 'person' && person.waitingOn.person === viewerName) return 'needs-you'
  if (person.state === 'away') return 'away'
  if (person.waitingOn !== undefined) return 'waiting'
  return person.state === 'working' ? 'working' : 'idle'
}
