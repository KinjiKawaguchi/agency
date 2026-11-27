// A clock for the tests that starts at a chosen moment and runs on from there, so that a test can be run as if it were another
// day (`AGENCY_TEST_NOW=2026-11-28T12:00:00Z npm test`, with `TZ=Asia/Tokyo` for another time zone). It finds the tests that say
// something true only on the day they were written: a number made up from the date, a week that has moved on, a zone that
// changed its hour. Without the variable nothing changes. A test that pins its own clock (`vi.useFakeTimers`, a node made
// with a `now`) is not moved, which is the point of pinning it.

interface DateHolder {
  Date: DateConstructor
}

/** Replaces the `Date` of `target` with one whose clock starts at `startAt` and runs on; a moment that is given to it is left as it is. Nothing when `startAt` is absent. */
export function installShiftedClock(target: DateHolder, startAt: string | undefined): void {
  if (startAt === undefined || startAt === '') return
  const RealDate = target.Date
  const start = RealDate.parse(startAt)
  if (Number.isNaN(start)) throw new Error(`AGENCY_TEST_NOW is not a time: ${startAt}`)
  const offset = start - RealDate.now()
  class ShiftedDate extends RealDate {
    constructor(...args: unknown[]) {
      if (args.length === 0) super(RealDate.now() + offset)
      else super(...(args as [number]))
    }

    static override now(): number {
      return RealDate.now() + offset
    }
  }
  target.Date = ShiftedDate as unknown as DateConstructor
}
