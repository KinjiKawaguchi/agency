/** What the viewer has typed or picked for one item, kept until it is sent. */
export interface Draft {
  text: string
  choice: string | null
}

export const EMPTY_DRAFT: Draft = { text: '', choice: null }

export type Drafts = ReadonlyMap<string, Draft>

export const draftOf = (drafts: Drafts, key: string): Draft => drafts.get(key) ?? EMPTY_DRAFT

export const withDraft = (drafts: Drafts, key: string, change: Partial<Draft>): Drafts =>
  new Map([...drafts, [key, { ...draftOf(drafts, key), ...change }]])

export const withoutDraft = (drafts: Drafts, key: string): Drafts => new Map([...drafts].filter(([one]) => one !== key))
