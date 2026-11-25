import type { PersonStatus } from '../api/teamview'

/** What the viewer needs to know about a person at a glance. */
export type Presence = 'needs-you' | 'working' | 'waiting' | 'idle' | 'away'

/**
 * `waitingOnViewer` is the node's list of the team's people who wait on the viewer (`TeamSnapshot.waitingOnViewer`).
 * `needs-you`: the person can only continue after the viewer acts.
 * `waiting`: they wait on someone else. `unknown` counts as idle; nothing is shown for it.
 */
export function presenceOf(person: PersonStatus, waitingOnViewer: ReadonlySet<string>): Presence {
  if (waitingOnViewer.has(person.id)) return 'needs-you'
  if (person.state === 'away') return 'away'
  if (person.waitingOn !== undefined) return 'waiting'
  return person.state === 'working' ? 'working' : 'idle'
}
