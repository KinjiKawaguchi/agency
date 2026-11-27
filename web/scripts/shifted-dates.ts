// Prints the moments the weekly run starts its clock at, one for each line: name, moment (UTC), time zone, separated by tabs.
//   node scripts/shifted-dates.ts [--today 2026-11-27T10:00:00Z]
import { shiftedRuns } from '../src/test/shiftedDates.ts'

const at = process.argv.indexOf('--today')
const today = at === -1 ? new Date() : new Date(process.argv[at + 1] ?? '')
if (Number.isNaN(today.getTime())) {
  console.error('--today is not a time')
  process.exit(2)
}
for (const run of shiftedRuns(today)) console.log(`${run.name}\t${run.now}\t${run.timeZone}`)
