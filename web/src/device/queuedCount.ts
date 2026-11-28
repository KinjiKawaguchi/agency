import { MOST_DECISIONS } from '../api/queries'

/** A log entry as far as the count reads it: where it was made, which device signed it (the node left that out before it kept it), and where it stands. */
interface CountedEntry {
  org: string
  signer?: string
  state: { kind: string }
}

/** The device being removed, by its id in each org it is listed in. */
type Listed = ReadonlyArray<{ org: string; deviceId: string }>

export interface QueuedCount {
  /** Held by the node (`queued`) and signed by the device being removed: the node refuses these once it is removed. */
  signedByDevice: number
  /** Held by the node, with no signer recorded: the node cannot say whether the device signed them. */
  notRecorded: number
  /** Held by the node and signed by another device: the removal does not touch them. */
  byOthers: number
  /** Whether the node gave all it gives, so older ones may be left out and each count is a floor. */
  isAtLeast: boolean
}

/**
 * How the decision log's undelivered entries divide by whether the device being removed signed them. The node gives at most `MOST_DECISIONS`
 * of the newest, so when the answer is full there may be more than were read, and every count is "at least".
 */
export function queuedCount(entries: ReadonlyArray<CountedEntry>, device: Listed): QueuedCount {
  const queued = entries.filter((entry) => entry.state.kind === 'queued')
  const signedByDevice = queued.filter((entry) => device.some((one) => one.org === entry.org && one.deviceId === entry.signer)).length
  const notRecorded = queued.filter((entry) => entry.signer === undefined).length
  return { signedByDevice, notRecorded, byOthers: queued.length - signedByDevice - notRecorded, isAtLeast: entries.length >= MOST_DECISIONS }
}
