import { MOST_DECISIONS } from '../api/queries'
import { queuedCount } from './queuedCount'

const laptop = [{ org: 'acme', deviceId: 'd-laptop' }]
const entry = (state: string, signer?: string, org = 'acme') => ({ org, state: { kind: state }, ...(signer === undefined ? {} : { signer }) })
const repeat = <T>(count: number, make: () => T): T[] => Array.from({ length: count }, make)

describe('how the undelivered decisions divide by who signed them', () => {
  it('counts what the device signed, what has no signer recorded, and what others signed, each apart', () => {
    const entries = [entry('queued', 'd-laptop'), entry('queued', 'd-laptop'), entry('queued'), entry('queued', 'd-phone'), entry('queued', 'd-phone'), entry('queued', 'd-phone')]
    expect(queuedCount(entries, laptop)).toEqual({ signedByDevice: 2, notRecorded: 1, byOthers: 3, isAtLeast: false })
    expect(queuedCount([], laptop)).toEqual({ signedByDevice: 0, notRecorded: 0, byOthers: 0, isAtLeast: false })
  })

  it('counts only what the node holds', () => {
    expect(queuedCount([entry('applied', 'd-laptop'), entry('delivered', 'd-laptop'), entry('refused', 'd-laptop'), entry('applied')], laptop)).toEqual({ signedByDevice: 0, notRecorded: 0, byOthers: 0, isAtLeast: false })
  })

  it('counts an id only in the org it is the device’s id in', () => {
    expect(queuedCount([entry('queued', 'd-laptop', 'club')], laptop)).toEqual({ signedByDevice: 0, notRecorded: 0, byOthers: 1, isAtLeast: false })
    expect(queuedCount([entry('queued', 'd-laptop', 'club')], [...laptop, { org: 'club', deviceId: 'd-laptop' }]).signedByDevice).toBe(1)
  })

  it('says it is at least that many when the node gave all it gives, since older ones may be left out', () => {
    expect(queuedCount(repeat(MOST_DECISIONS - 1, () => entry('queued', 'd-laptop')), laptop).isAtLeast).toBe(false)
    expect(queuedCount(repeat(MOST_DECISIONS, () => entry('queued', 'd-laptop')), laptop)).toEqual({ signedByDevice: MOST_DECISIONS, notRecorded: 0, byOthers: 0, isAtLeast: true })
    expect(queuedCount([...repeat(4, () => entry('queued')), ...repeat(MOST_DECISIONS - 4, () => entry('applied'))], laptop).isAtLeast).toBe(true)
  })

  it('is 200, the most the node gives in one read', () => {
    expect(MOST_DECISIONS).toBe(200)
  })
})
