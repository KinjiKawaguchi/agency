// The Client API between the web client and its home node. These shapes are not generated from
// Rust yet; they are the web client's side of the calls and stay small on purpose.
import type { AnswerWith } from './teamview'

/** The signed-in person and the organizations and teams they can reach through their home node. */
export interface Me {
  account: { id: string; label: string }
  orgs: Array<{
    id: string
    label: string
    teams: Array<{ id: string; label: string }>
  }>
}

/** One decision: which request it answers (`answerWith` is copied from the request) and what was decided. */
export interface DecisionAnswer {
  org: string
  team: string
  answerWith: AnswerWith
  choice?: string
  text: string
}
