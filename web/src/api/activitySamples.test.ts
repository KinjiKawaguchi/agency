import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { readActivity } from './activityAnswers'

// The node's own samples (docs/protocol.md) are what the client has to read: a valid answer needs no mending, so a
// change of shape in the node fails here. The tests run from `web/`, next to `testdata/`.
const sample = (name: string): unknown => JSON.parse(readFileSync(join(process.cwd(), '..', 'testdata', name), 'utf8'))

describe('the node’s samples of the activity', () => {
  it('read unchanged, whether the shared sample or what a node answered in the client fixture', () => {
    for (const name of ['activity.sample.json', 'client/activity.json']) {
      const answer = sample(name) as { events: unknown[]; truncated?: boolean }
      const read = readActivity(answer)
      expect(read.events.length).toBe(answer.events.length)
      expect(JSON.parse(JSON.stringify(read.events))).toEqual(answer.events)
      expect(read.truncated).toBe(answer.truncated === true)
    }
  })

  it('has every kind of event the screen shows', () => {
    const kinds = new Set(readActivity(sample('activity.sample.json')).events.map((event) => event.kind))
    for (const kind of ['itemAdded', 'statusChanged', 'prOpened', 'prMerged', 'message']) expect(kinds).toContain(kind)
  })
})
