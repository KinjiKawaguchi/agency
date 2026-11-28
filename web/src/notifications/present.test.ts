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
    expect(noticesFor([e('request', `request:acme/app/${request.id}`)], lookups, en, on)).toEqual([{ tone: 'info', target: { to: '/desk', item: `acme:app:${request.id}` }, tag: `request/acme/app/${request.id}`, message: `A request waits for you: ${request.title} (${sampleTeamView.label}).`, plain: 'A request is waiting for you' }])
    expect(noticesFor([e('request', `request:acme/app/${request.id}`), e('request', 'request:acme/app/r-2')], lookups, en, on)[0]?.message).toBe('2 requests now wait for you.')
  })

  it('names a participant who waits, and a work item that is blocked, by their labels and titles', () => {
    const person = sampleTeamView.people[1]!
    expect(noticesFor([e('participant', `participant:acme/app/${person.id}`)], lookups, en, on)[0]?.message).toBe(`${person.label} is waiting for you.`)
    expect(noticesFor([e('blocked', `blocked:acme/app/${blockedItem.ref}`)], lookups, en, on)[0]?.message).toBe(`Blocked: ${blockedItem.title} (${sampleTeamView.label}): ${en.people.blockedReasons[blockedItem.blocked!.reason]}.`)
  })

  it('names a device that asks, and says nothing of it on the devices screen, which lists devices itself', () => {
    expect(noticesFor([e('device', 'device:k1')], lookups, en, on)[0]?.message).toBe('Phone asks to be added as a device.')
    expect(noticesFor([e('device', 'device:k1')], lookups, en, { onDevices: true })).toEqual([])
  })

  it('shows a refused decision as an error with the node’s reason, and a queued one as news', () => {
    expect(noticesFor([e('decision', 'decision:d1:refused')], lookups, en, on)).toEqual([{ tone: 'error', target: { to: '/desk' }, tag: 'decision/d1', message: 'A decision “Add a second frontend agent” was refused: Not the right team.', plain: 'A decision was refused' }])
    expect(noticesFor([e('decision', 'decision:d2:queued')], lookups, en, on)[0]).toMatchObject({ tone: 'info', message: 'A decision “Rename the export endpoint” is saved, not delivered: its node cannot be reached yet.' })
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
    expect(noticesFor([e('device', 'device:k1')], lookups, ja, on)[0]?.message).toBe('端末「Phone」が追加を待っています')
    expect(noticesFor([e('decision', 'decision:d1:refused')], lookups, ja, on)[0]?.message).toBe('「Add a second frontend agent」の決定は断られました: Not the right team.')
  })

  it('says in Japanese what the iPhone banner says, kind by kind', () => {
    // The words of apple/AgencyKit/Sources/AgencyModel/Wording/JaNews.swift: both clients tell the same news the same way.
    const person = sampleTeamView.people[1]!
    const at = (kind: NoticeEvent['kind'], key: string, count = 1): string[] => noticesFor(Array.from({ length: count }, (_, index) => e(kind, count === 1 ? key : `${key}-${index}`)), lookups, ja, on).map((one) => one.message)
    expect(at('request', `request:acme/app/${request.id}`)).toEqual([`${sampleTeamView.label}: 「${request.title}」があなたを待っています`])
    expect(at('request', 'request:acme/app/r', 3)).toEqual(['3 件のリクエストがあなたを待っています'])
    expect(at('participant', `participant:acme/app/${person.id}`)).toEqual([`${person.label} があなたの返事を待っています`])
    expect(at('participant', 'participant:acme/app/p', 2)).toEqual(['メンバー 2 人があなたの返事を待っています'])
    expect(at('blocked', `blocked:acme/app/${blockedItem.ref}`)).toEqual([`${sampleTeamView.label}: 「${blockedItem.title}」が止まりました (${ja.people.blockedReasons[blockedItem.blocked!.reason]})`])
    expect(at('blocked', 'blocked:acme/app/x', 2)).toEqual(['2 件の仕事が止まりました'])
    expect(at('device', 'device:k1')).toEqual(['端末「Phone」が追加を待っています'])
    expect(at('device', 'device:k', 2)).toEqual(['2 台の端末が追加を待っています'])
    expect(at('decision', 'decision:d2:queued')).toEqual(['「Rename the export endpoint」の決定は保存済みですが、まだ届いていません'])
    expect(at('decision', 'decision:z:applied')).toEqual(['決定が反映されました'])
    expect(at('decision', 'decision:z:delivered')).toEqual(['決定が届きました'])
  })
})

describe('where a notice leads', () => {
  const targetOf = (event: NoticeEvent) => noticesFor([event], lookups, en, on)[0]?.target

  it('leads one request to that request on the desk, and several to the desk', () => {
    expect(targetOf(e('request', `request:acme/app/${request.id}`))).toEqual({ to: '/desk', item: `acme:app:${request.id}` })
    expect(noticesFor([e('request', 'request:acme/app/r-1'), e('request', 'request:acme/app/r-2')], lookups, en, on)[0]?.target).toEqual({ to: '/desk' })
  })

  it('keeps an id that holds a slash whole, as the desk names it', () => {
    expect(targetOf(e('request', 'request:acme/app/r-1/x'))).toEqual({ to: '/desk', item: 'acme:app:r-1/x' })
  })

  it('leads a participant who waits and a blocked item to the participants and work, a device to the devices, a decision to the desk', () => {
    expect(targetOf(e('participant', `participant:acme/app/${sampleTeamView.people[1]!.id}`))).toEqual({ to: '/people' })
    expect(targetOf(e('blocked', `blocked:acme/app/${blockedItem.ref}`))).toEqual({ to: '/people' })
    expect(targetOf(e('device', 'device:k1'))).toEqual({ to: '/devices' })
    expect(targetOf(e('decision', 'decision:d1:refused'))).toEqual({ to: '/desk' })
  })
})

describe('what a notice is named by, so that a browser shows one notification for one event', () => {
  const tagOf = (events: NoticeEvent[]) => noticesFor(events, lookups, en, on)[0]?.tag

  it('names one request by its organization, team, and id, and one decision by its id, and a device by its kind', () => {
    expect(tagOf([e('request', `request:acme/app/${request.id}`)])).toBe(`request/acme/app/${request.id}`)
    expect(tagOf([e('decision', 'decision:d1:refused')])).toBe('decision/d1')
    expect(tagOf([e('device', 'device:k1')])).toBe('device')
  })

  it('gives a notice of several events none, and one that no worker is told of none', () => {
    expect(tagOf([e('request', 'request:acme/app/r-1'), e('request', 'request:acme/app/r-2')])).toBeUndefined()
    expect(tagOf([e('device', 'device:k1'), e('device', 'device:k2')])).toBeUndefined()
    expect(tagOf([e('participant', `participant:acme/app/${sampleTeamView.people[1]!.id}`)])).toBeUndefined()
    expect(tagOf([e('blocked', `blocked:acme/app/${blockedItem.ref}`)])).toBeUndefined()
  })
})

describe('what a notice says when no title may leave the page', () => {
  const person = sampleTeamView.people[1]!
  const events = [
    e('request', `request:acme/app/${request.id}`),
    e('participant', `participant:acme/app/${person.id}`),
    e('blocked', `blocked:acme/app/${blockedItem.ref}`),
    e('device', 'device:k1'),
    e('decision', 'decision:d1:refused'),
    e('decision', 'decision:d2:queued'),
  ]
  const secrets = [request.title, sampleTeamView.label, person.label, blockedItem.title, 'Phone', 'Add a second frontend agent', 'Rename the export endpoint', 'Not the right team.', en.people.blockedReasons[blockedItem.blocked!.reason]]

  it.each([['English', en], ['Japanese', ja]] as const)('has no title, label, team, or reason in it (%s)', (_language, messages) => {
    const notices = events.flatMap((event) => noticesFor([event], lookups, messages, on))
    expect(notices).toHaveLength(events.length)
    for (const notice of notices) {
      for (const secret of secrets) expect(notice.plain, `${notice.message} -> ${notice.plain}`).not.toContain(secret)
    }
    // The page's own words still name them.
    expect(notices[0]?.message).toContain(request.title)
  })

  it('says what the phone says for a push whose titles are hidden', () => {
    expect(noticesFor([events[0]!], lookups, en, on)[0]?.plain).toBe('A request is waiting for you')
    expect(noticesFor([events[3]!], lookups, en, on)[0]?.plain).toBe('A device is waiting to be added')
    expect(noticesFor([events[4]!], lookups, en, on)[0]?.plain).toBe('A decision was refused')
    expect(noticesFor([events[0]!], lookups, ja, on)[0]?.plain).toBe('あなたを待っているリクエストがあります')
    expect(noticesFor([events[3]!], lookups, ja, on)[0]?.plain).toBe('端末が追加を待っています')
    expect(noticesFor([events[4]!], lookups, ja, on)[0]?.plain).toBe('決定が断られました')
  })

  it('keeps a count as it is: it names nothing', () => {
    const several = noticesFor([e('request', 'request:acme/app/r-1'), e('request', 'request:acme/app/r-2')], lookups, en, on)[0]
    expect(several?.plain).toBe('2 requests now wait for you.')
    expect(several?.plain).toBe(several?.message)
  })
})
