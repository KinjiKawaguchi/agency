import type { Messages } from '../i18n/en'
import { SignInExpiredError } from './session'

/** What to tell the viewer where a call to the node failed. The banner above the page already says the sign-in expired and offers to renew it. */
export const failureText = (t: Messages, error: Error): string =>
  error instanceof SignInExpiredError ? t.app.signInExpired.nothingToShow : t.app.unreachable(error.message)
