import { mkdtempSync, readFileSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

// The report file is read when the module is first loaded, so it is chosen before that.
const report = join(mkdtempSync(join(tmpdir(), 'flaky-')), 'flaky-tests.txt')
process.env.FLAKY_REPORT = report

const read = () => (existsSync(report) ? readFileSync(report, 'utf8') : '')

describe('the reports of the tests that passed only on a retry', () => {
  it('writes down a unit test that passed after a retry, and not one that passed first time or failed', async () => {
    const { default: FlakyReporter } = await import('./flakyReporter')
    const reporter = new FlakyReporter()
    const test = (flaky: boolean | undefined) =>
      ({ diagnostic: () => (flaky === undefined ? undefined : { flaky }), module: { moduleId: '/ci/work/web/src/device/Page.test.tsx' }, fullName: 'screen > shows it' }) as never
    reporter.onTestCaseResult(test(false))
    reporter.onTestCaseResult(test(undefined))
    expect(read()).toBe('')
    reporter.onTestCaseResult(test(true))
    expect(read()).toBe('web: src/device/Page.test.tsx > screen > shows it\n')
  })

  it('writes down an end-to-end test whose outcome is flaky, once it has passed', async () => {
    const { default: FlakyReporter } = await import('../../e2e/flakyReporter')
    const reporter = new FlakyReporter()
    const test = (outcome: string) => ({ outcome: () => outcome, titlePath: () => ['', 'chromium', 'desk.spec.ts', 'the desk', 'opens a request'] }) as never
    reporter.onTestEnd(test('flaky'), { status: 'failed' } as never)
    reporter.onTestEnd(test('expected'), { status: 'passed' } as never)
    expect(read()).not.toContain('e2e:')
    reporter.onTestEnd(test('flaky'), { status: 'passed' } as never)
    expect(read()).toContain('e2e: chromium > desk.spec.ts > the desk > opens a request\n')
  })
})
