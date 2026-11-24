// The only module that reaches into `bindings/` (types generated from the Rust types) and
// `testdata/` (the sample TeamView the Rust tests, the web client, and the harness share).
// Never edit the bindings by hand; regenerate them from Rust.
import type { TeamView } from '../../../bindings/TeamView'
import sample from '../../../testdata/teamview.sample.json' with { type: 'json' }

export type { AnswerWith } from '../../../bindings/AnswerWith'
export type { Blocked } from '../../../bindings/Blocked'
export type { BlockedReason } from '../../../bindings/BlockedReason'
export type { EpicForecast } from '../../../bindings/EpicForecast'
export type { Eta } from '../../../bindings/Eta'
export type { PersonKind } from '../../../bindings/PersonKind'
export type { PersonState } from '../../../bindings/PersonState'
export type { PersonStatus } from '../../../bindings/PersonStatus'
/** `Request` is also the name of the DOM fetch type, so the app calls the team's request `TeamRequest`. */
export type { Request as TeamRequest } from '../../../bindings/Request'
export type { RequestKind } from '../../../bindings/RequestKind'
export type { RequestOption } from '../../../bindings/RequestOption'
export type { TeamView } from '../../../bindings/TeamView'
export type { ThreadMessage } from '../../../bindings/ThreadMessage'
export type { WaitingOn } from '../../../bindings/WaitingOn'
export type { WorkItem } from '../../../bindings/WorkItem'
export type { WorkStatus } from '../../../bindings/WorkStatus'

/** The shared sample, exactly as checked in. */
export const sampleTeamView: TeamView = sample as TeamView
