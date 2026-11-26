import type { Reporter, TestCase, TestResult } from '@playwright/test/reporter'
import { recordFlaky } from '../src/test/flakyReport'

/** Writes down each end-to-end test that passed only on a retry, for the summary of the CI run. */
export default class FlakyReporter implements Reporter {
  onTestEnd(test: TestCase, result: TestResult): void {
    if (result.status === 'passed' && test.outcome() === 'flaky') recordFlaky('e2e', test.titlePath().filter((part) => part !== '').join(' > '))
  }
}
