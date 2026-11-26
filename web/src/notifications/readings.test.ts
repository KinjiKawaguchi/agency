import type { DecisionLog, DeviceList } from '../api/clientApi'
import { sampleTeamView } from '../api/teamview'
import { teamOf } from '../test/views'
import { advance, START, type Own } from './engine'
import { decisionStatesOf, deviceKeysOf, teamReadingOf, teamReadings, unreadTeam } from './readings'

const own: Own = { deviceKey: 'own', canApprove: true, sentDecisionIds: [] }

describe('reading what the node answered', () => {
  it('reads a team’s requests, waiting participants, and blocked work as the node lists them', () => {
    const team = teamOf(sampleTeamView)
    const reading = teamReadingOf(team)
    expect(reading).toMatchObject({ org: 'acme', team: 'app', ok: true, mine: [...team.mine], waitingOnViewer: [...team.waitingOnViewer] })
    const blocked = sampleTeamView.work.filter((item) => item.blocked !== undefined)
    expect(blocked.length).toBeGreaterThan(0)
    expect(reading.blocked.map((item) => item.ref)).toEqual(blocked.map((item) => item.ref))
    expect(reading.blocked[0]?.reason).toBe(blocked[0]?.blocked?.reason)
  })

  it('counts a team that failed, or is still loading, as not read, and one that is ready as read', () => {
    const ready = teamOf(sampleTeamView)
    const readings = teamReadings([
      {
        org: { id: 'acme' },
        teams: [
          { ref: { id: 'app' }, state: { status: 'ready', team: ready } },
          { ref: { id: 'site' }, state: { status: 'failed', error: new Error('no'), at: 0 } },
          { ref: { id: 'docs' }, state: { status: 'loading' } },
        ],
      },
    ])
    expect(readings.map((one) => [one.team, one.ok])).toEqual([['app', true], ['site', false], ['docs', false]])
    expect(readings[1]).toEqual(unreadTeam('acme', 'site'))
  })

  it('reads the keys of the devices that wait, and nothing when the list is not there', () => {
    const list = { orgs: [], requests: [{ label: 'Phone', publicKey: 'k1', deviceId: 'd-1', pairingCode: 'a', requestedAt: '2026-11-24T12:00:00Z' }] } as DeviceList
    expect(deviceKeysOf(list)).toEqual(['k1'])
    expect(deviceKeysOf(undefined)).toBeNull()
  })

  it('reads each decision by its id and the kind of its state, and nothing when the log is not there', () => {
    const log = { entries: [{ id: 'd1', state: { kind: 'applied' } }, { id: 'd2', state: { kind: 'queued', attempts: 1, nextAt: '2026-11-24T12:00:00Z' } }] } as DecisionLog
    expect(decisionStatesOf(log)).toEqual([{ id: 'd1', state: 'applied' }, { id: 'd2', state: 'queued' }])
    expect(decisionStatesOf(undefined)).toBeNull()
  })

  it('makes no event when a team that failed comes back with what it had, and one for the request that arrived meanwhile', () => {
    const before = teamOf(sampleTeamView)
    const first = advance(START, { teams: teamReadings([{ org: { id: 'acme' }, teams: [{ ref: { id: 'app' }, state: { status: 'ready', team: before } }] }]), devices: [], decisions: [] }, own)
    const failed = advance(first.state, { teams: teamReadings([{ org: { id: 'acme' }, teams: [{ ref: { id: 'app' }, state: { status: 'failed', error: new Error('no'), at: 0 } }] }]), devices: null, decisions: null }, own)
    expect(failed.events).toEqual([])
    const again = advance(failed.state, { teams: teamReadings([{ org: { id: 'acme' }, teams: [{ ref: { id: 'app' }, state: { status: 'ready', team: before } }] }]), devices: [], decisions: [] }, own)
    expect(again.events).toEqual([])
  })
})
