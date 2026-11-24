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

describe('presenceOf', () => {
  it('needs you when only the owner can unblock them, even while the session is working', () => {
    expect(presenceOf(person({ state: 'working', waitingOn: { kind: 'person', person: 'owner' } }))).toBe('needs-you')
  })

  it('waits on someone else when the lead or another person has to act', () => {
    expect(presenceOf(person({ state: 'idle', waitingOn: { kind: 'lead' } }))).toBe('waiting')
    expect(presenceOf(person({ waitingOn: { kind: 'person', person: 'frontend' } }))).toBe('waiting')
  })

  it('reports away before anything else that is not the owner', () => {
    expect(presenceOf(person({ state: 'away' }))).toBe('away')
  })

  it('treats working as working and unknown as idle', () => {
    expect(presenceOf(person({ state: 'working' }))).toBe('working')
    expect(presenceOf(person({ state: 'unknown' }))).toBe('idle')
    expect(presenceOf(person({ state: 'idle' }))).toBe('idle')
  })
})
