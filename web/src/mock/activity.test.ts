import { sampleTeamView } from '../api/teamview'
import { activityFor } from './activity'

const NOW = Date.parse('2026-11-26T10:00:00Z')
const HOUR = 3_600_000

describe('the mock activity feed', () => {
  const hour = activityFor(sampleTeamView, NOW - HOUR, NOW)

  it('is oldest first, strictly inside the window, and has a unique id for each event', () => {
    const times = hour.events.map((event) => Date.parse(event.at))
    expect(times).toEqual(times.toSorted((a, b) => a - b))
    expect(times.every((time) => time > NOW - HOUR && time <= NOW)).toBe(true)
    expect(new Set(hour.events.map((event) => event.id)).size).toBe(hour.events.length)
  })

  it('tells one story: a later read of a window overlaps the earlier one with the same events', () => {
    const later = activityFor(sampleTeamView, NOW - HOUR / 2, NOW + 60_000)
    const earlier = new Set(hour.events.filter((event) => Date.parse(event.at) > NOW - HOUR / 2).map((event) => event.id))
    const shared = later.events.filter((event) => earlier.has(event.id))
    expect(shared.length).toBe(earlier.size)
  })

  it('has the kinds the screen shows, among the team’s own agents, and sometimes an empty summary', () => {
    const agents = new Set(sampleTeamView.people.filter((person) => person.kind === 'agent').map((person) => person.id))
    const kinds = new Set(hour.events.map((event) => event.kind))
    expect(kinds).toContain('message')
    expect(kinds).toContain('statusChanged')
    for (const event of hour.events) if (event.kind === 'message') expect([agents.has(event.from), agents.has(event.to), event.from !== event.to]).toEqual([true, true, true])
    expect(hour.events.filter((event) => event.kind === 'message').length).toBeGreaterThan(50)
  })

  it('is cut at the limit, oldest first, and says so', () => {
    const cut = activityFor(sampleTeamView, NOW - HOUR, NOW, 5)
    expect(cut.events).toHaveLength(5)
    expect(cut.truncated).toBe(true)
    expect(cut.events.map((event) => event.id)).toEqual(hour.events.slice(0, 5).map((event) => event.id))
  })

  it('merges a few pull requests an hour, as a team of this size would', () => {
    const merged = hour.events.filter((event) => event.kind === 'prMerged').length
    expect(merged).toBeGreaterThan(0)
    expect(merged).toBeLessThan(20)
  })
})
