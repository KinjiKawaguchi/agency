import type { DecisionLog, DeviceList } from '../api/clientApi'
import { sampleTeamView } from '../api/teamview'
import { en } from '../i18n/en'
import { ja } from '../i18n/ja'
import { teamOf } from '../test/views'
import type { NoticeEvent } from './engine'
import { noticesFor, type Lookups } from './present'

const team = teamOf(sampleTeamView)
const request = sampleTeamView.requests[0]!
const blockedItem = sampleTeamView.work.find((item) => item.blocked !== undefined)!
const devices = { orgs: [], requests: [{ label: 'Phone', publicKey: 'k1', deviceId: 'd-1', pairingCode: 'a', requestedAt: '2026-11-24T12:00:00Z' }] } as DeviceList
const decisions = {
  entries: [
    { id: 'd1', title: 'Add a second frontend agent', state: { kind: 'refused', message: 'Not the right team.' } },
    { id: 'd2', title: 'Rename the export endpoint', state: { kind: 'queued', attempts: 0, nextAt: '2026-11-24T12:00:00Z' } },
  ],
} as DecisionLog
const lookups: Lookups = { teams: [team], devices, decisions }
const on = { onDevices: false }
const e = (kind: NoticeEvent['kind'], key: string): NoticeEvent => ({ kind, key })

describe('what the viewer is told', () => {
  it('names a request, with its team, when one arrives, and counts them when several do', () => {
    expect(noticesFor([e('request', `request:acme/app/${request.id}`)], lookups, en, on)).toEqual([{ tone: 'info', message: `A request waits for you: ${request.title} (${sampleTeamView.label}).` }])
    expect(noticesFor([e('request', `request:acme/app/${request.id}`), e('request', 'request:acme/app/r-2')], lookups, en, on)[0]?.message).toBe('2 requests now wait for you.')
  })

  it('names a participant who waits, and a work item that is blocked, by their labels and titles', () => {
    const person = sampleTeamView.people[1]!
    expect(noticesFor([e('participant', `participant:acme/app/${person.id}`)], lookups, en, on)[0]?.message).toBe(`${person.label} is waiting for you.`)
    expect(noticesFor([e('blocked', `blocked:acme/app/${blockedItem.ref}`)], lookups, en, on)[0]?.message).toBe(`Blocked: ${blockedItem.title} (${sampleTeamView.label}).`)
  })

  it('names a device that asks, and says nothing of it on the devices screen, which lists devices itself', () => {
    expect(noticesFor([e('device', 'device:k1')], lookups, en, on)[0]?.message).toBe('Phone asks to be added as a device.')
    expect(noticesFor([e('device', 'device:k1')], lookups, en, { onDevices: true })).toEqual([])
  })

  it('shows a refused decision as an error with the node’s reason, and a queued one as news', () => {
    expect(noticesFor([e('decision', 'decision:d1:refused')], lookups, en, on)).toEqual([{ tone: 'error', message: 'A decision “Add a second frontend agent” was refused: Not the right team.' }])
    expect(noticesFor([e('decision', 'decision:d2:queued')], lookups, en, on)[0]).toMatchObject({ tone: 'info', message: 'A decision “Rename the export endpoint” is queued: its node cannot be reached yet.' })
  })

  it('does not break on an id with a colon, or on what it has no lookup for', () => {
    expect(noticesFor([e('decision', 'decision:x:y:applied')], { teams: [], devices: undefined, decisions: undefined }, en, on)[0]?.message).toBe('A decision was applied.')
    expect(noticesFor([e('request', 'request:lab/none/r-9')], { teams: [], devices: undefined, decisions: undefined }, en, on)[0]?.message).toBe('A request waits for you: r-9 (none).')
  })

  it('shows at most three, and counts the rest in the last', () => {
    const events = [e('request', 'request:acme/app/r-1'), e('participant', 'participant:acme/app/backend'), e('blocked', `blocked:acme/app/${blockedItem.ref}`), e('device', 'device:k1'), e('decision', 'decision:d2:queued')]
    const shown = noticesFor(events, lookups, en, on)
    expect(shown).toHaveLength(3)
    expect(shown[2]?.message).toBe('3 more things are new.')
  })

  it('says nothing when nothing is new', () => {
    expect(noticesFor([], lookups, en, on)).toEqual([])
  })

  it('reads in Japanese', () => {
    expect(noticesFor([e('device', 'device:k1')], lookups, ja, on)[0]?.message).toBe('Phone が、端末として追加されるのを待っています。')
    expect(noticesFor([e('decision', 'decision:d1:refused')], lookups, ja, on)[0]?.message).toBe('決定「Add a second frontend agent」が受け付けられませんでした: Not the right team.')
  })
})
