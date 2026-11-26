import type { Reporter, TestCase } from 'vitest/node'
import { recordFlaky } from './flakyReport'

/** Writes down each test that passed only on a retry, for the summary of the CI run. */
export default class FlakyReporter implements Reporter {
  onTestCaseResult(testCase: TestCase): void {
    if (testCase.diagnostic()?.flaky === true) recordFlaky('web', `${testCase.module.moduleId.split('/web/')[1] ?? testCase.module.moduleId} > ${testCase.fullName}`)
  }
}
