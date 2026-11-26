import { appendFileSync } from 'node:fs'

/** Where the flaky tests of this run are written, one line each; the CI summary reads it. */
const REPORT = process.env.FLAKY_REPORT ?? 'flaky-tests.txt'

/** Notes that a test failed and then passed on a retry. */
export function recordFlaky(runner: string, name: string): void {
  appendFileSync(REPORT, `${runner}: ${name}\n`)
}
