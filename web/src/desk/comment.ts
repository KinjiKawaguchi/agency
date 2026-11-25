import type { Draft } from './drafts'

/**
 * Text with no option picked, on a document that has options, is a comment: the turn goes back to the
 * team and nothing is decided. A document with no options takes its text as the answer.
 */
export const isComment = (optionCount: number, draft: Draft): boolean => optionCount > 0 && draft.choice === null && draft.text.trim() !== ''
