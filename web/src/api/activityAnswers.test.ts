import { MalformedAnswer } from './evaluationAnswers'
import { readActivity } from './activityAnswers'

const header = { org: 'acme', team: 'app', generatedAt: '2026-11-26T10:00:00Z' }
const merged = { id: 'b-1', at: '2026-11-26T09:59:00Z', kind: 'prMerged', ref: 'app#7', title: 'Fix it', leadSeconds: 5400 }

describe('readActivity', () => {
  it('reads each kind of event, oldest first, with an optional field left out when the node leaves it out', () => {
    const activity = readActivity({
      ...header,
      events: [
        { id: 'm-2', at: '2026-11-26T09:59:30Z', kind: 'message', from: 'lead', to: 'backend', summary: 'Which database?' },
        { id: 'b-3', at: '2026-11-26T09:58:00Z', kind: 'itemAdded', ref: 'app#9', title: 'New thing', epic: 'app#1' },
        { id: 'b-4', at: '2026-11-26T09:58:30Z', kind: 'statusChanged', ref: 'app#9', title: 'New thing', from: 'next', to: 'inProgress' },
        { id: 'b-5', at: '2026-11-26T09:58:45Z', kind: 'prOpened', ref: 'app#10', title: 'Fix it', issue: 'app#9' },
        merged,
      ],
      truncated: false,
    })
    expect(activity.events.map((event) => event.id)).toEqual(['b-3', 'b-4', 'b-5', 'b-1', 'm-2'])
    const opened = activity.events.find((event) => event.id === 'b-5')
    expect(opened && 'by' in opened).toBe(false)
    expect(activity.events.find((event) => event.id === 'b-3')).toMatchObject({ epic: 'app#1' })
  })

  it('leaves out an event of a kind it does not know, and one that is malformed, without spoiling the rest', () => {
    const activity = readActivity({
      ...header,
      events: [
        { id: 'x-1', at: '2026-11-26T09:00:00Z', kind: 'deploy', ref: 'app#1' },
        { id: 'b-1', at: 'yesterday', kind: 'prMerged', ref: 'app#1', title: 'Bad time' },
        { id: 'b-2', at: '2026-11-26T09:00:00Z', kind: 'statusChanged', ref: 'app#1', title: 'Bad status', from: 'next', to: 'flying' },
        { at: '2026-11-26T09:00:00Z', kind: 'message', from: 'a', to: 'b', summary: 'no id' },
        merged,
      ],
    })
    expect(activity.events.map((event) => event.id)).toEqual(['b-1'])
    expect(activity.truncated).toBe(false)
  })

  it('cuts a summary at 120 characters, keeps an empty one empty, and never reads a body', () => {
    const activity = readActivity({ ...header, events: [{ id: 'm-1', at: '2026-11-26T09:00:00Z', kind: 'message', from: 'a', to: 'b', summary: 'x'.repeat(300), message: 'the body' }, { id: 'm-2', at: '2026-11-26T09:00:01Z', kind: 'message', from: 'a', to: 'b' }] })
    const [long, blank] = activity.events
    expect(long && 'summary' in long ? long.summary : '').toHaveLength(120)
    expect(long && 'message' in long).toBe(false)
    expect(blank && 'summary' in blank ? blank.summary : 'x').toBe('')
  })

  it('refuses an answer with no team or time, and says it was cut when it was', () => {
    expect(() => readActivity({ org: 'acme' })).toThrow(MalformedAnswer)
    expect(readActivity({ ...header, truncated: true }).truncated).toBe(true)
  })
})
