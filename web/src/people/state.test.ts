import type { PersonStatus } from '../api/teamview'
import { presenceOf } from './state'

const person = (change: Partial<PersonStatus>): PersonStatus => ({
  id: 'backend',
  label: 'Backend',
  kind: 'agent',
  role: 'member',
  state: 'working',
  status: '',
  holding: [],
  ...change,
})

const NONE: ReadonlySet<string> = new Set()
const BACKEND: ReadonlySet<string> = new Set(['backend'])

describe('presenceOf', () => {
  it('needs you when the node lists them as waiting on the viewer, even while the session is working', () => {
    expect(presenceOf(person({ state: 'working', waitingOn: { kind: 'person', person: 'owner' } }), BACKEND)).toBe('needs-you')
  })

  it('goes by the node’s list, not by whom `waitingOn` seems to name', () => {
    // Names the owner, but the node does not list them: the team does not address this viewer.
    expect(presenceOf(person({ state: 'working', waitingOn: { kind: 'person', person: 'owner' } }), NONE)).toBe('waiting')
    // Listed, whatever `waitingOn` says.
    expect(presenceOf(person({ state: 'idle' }), BACKEND)).toBe('needs-you')
  })

  it('looks at the person’s id, so someone else’s listing does not make them need you', () => {
    expect(presenceOf(person({ id: 'frontend', waitingOn: { kind: 'person', person: 'owner' } }), BACKEND)).toBe('waiting')
  })

  it('waits on someone else when the lead or another person has to act', () => {
    expect(presenceOf(person({ state: 'idle', waitingOn: { kind: 'lead' } }), NONE)).toBe('waiting')
    expect(presenceOf(person({ waitingOn: { kind: 'person', person: 'frontend' } }), NONE)).toBe('waiting')
  })

  it('reports away before anything else that is not the viewer', () => {
    expect(presenceOf(person({ state: 'away' }), NONE)).toBe('away')
  })

  it('treats working as working and unknown as idle', () => {
    expect(presenceOf(person({ state: 'working' }), NONE)).toBe('working')
    expect(presenceOf(person({ state: 'unknown' }), NONE)).toBe('idle')
    expect(presenceOf(person({ state: 'idle' }), NONE)).toBe('idle')
  })
})
