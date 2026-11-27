// The Client API between the web client and its home node (docs/protocol.md, "Client API").
// Its types are generated from the Rust types in `bindings/` and are never edited by hand. The only
// types defined here are the client's own: the arguments of a decision call.
import type { AnswerWith } from './teamview'

export type { About } from '../../../bindings/About'
export type { AccountRef } from '../../../bindings/AccountRef'
export type { DayCost } from '../../../bindings/DayCost'
export type { Device } from '../../../bindings/Device'
export type { DeviceChange } from '../../../bindings/DeviceChange'
export type { DeviceChangeResult } from '../../../bindings/DeviceChangeResult'
export type { DeviceList } from '../../../bindings/DeviceList'
export type { DeviceRequest } from '../../../bindings/DeviceRequest'
export type { DeviceRequestInput } from '../../../bindings/DeviceRequestInput'
export type { NodeInfo } from '../../../bindings/NodeInfo'
export type { Envelope } from '../../../bindings/Envelope'
export type { Signed } from '../../../bindings/Signed'
export type { PushEnvironment } from '../../../bindings/PushEnvironment'
export type { PushToken } from '../../../bindings/PushToken'
export type { PushTokenInput } from '../../../bindings/PushTokenInput'
export type { ObjectiveResult } from '../../../bindings/ObjectiveResult'
export type { ObjectiveState } from '../../../bindings/ObjectiveState'
export type { PushFailure } from '../../../bindings/PushFailure'
export type { PushSubscription } from '../../../bindings/PushSubscription'
export type { PushSubscriptionInput } from '../../../bindings/PushSubscriptionInput'
export type { PushSubscriptionList } from '../../../bindings/PushSubscriptionList'
export type { VapidKey } from '../../../bindings/VapidKey'
export type { SignedDecisionInput } from '../../../bindings/SignedDecisionInput'
export type { Session } from '../../../bindings/Session'
export type { DecisionEntry } from '../../../bindings/DecisionEntry'
export type { DecisionLog } from '../../../bindings/DecisionLog'
export type { DecisionOutcome } from '../../../bindings/DecisionOutcome'
export type { DecisionResult } from '../../../bindings/DecisionResult'
export type { DecisionState } from '../../../bindings/DecisionState'
export type { Department } from '../../../bindings/Department'
export type { Funding } from '../../../bindings/Funding'
export type { FundingKind } from '../../../bindings/FundingKind'
export type { Me } from '../../../bindings/Me'
export type { OrgDefinition } from '../../../bindings/OrgDefinition'
export type { OrgRef } from '../../../bindings/OrgRef'
export type { RootChangeKind } from '../../../bindings/RootChangeKind'
export type { RootChangeRef } from '../../../bindings/RootChangeRef'
export type { StartedRecovery } from '../../../bindings/StartedRecovery'
export type { DeviceChangeOutcome } from '../../../bindings/DeviceChangeOutcome'
export type { Person } from '../../../bindings/Person'
export type { SnapshotSource } from '../../../bindings/SnapshotSource'
export type { TeamLocation } from '../../../bindings/TeamLocation'
export type { TeamPlace } from '../../../bindings/TeamPlace'
export type { TeamRef } from '../../../bindings/TeamRef'
export type { TeamSnapshot } from '../../../bindings/TeamSnapshot'
export type { PersonUsage } from '../../../bindings/PersonUsage'
export type { TokenCounts } from '../../../bindings/TokenCounts'
export type { TokenUsage } from '../../../bindings/TokenUsage'
export type { UsageWindow } from '../../../bindings/UsageWindow'
export type { UsageWindowName } from '../../../bindings/UsageWindowName'

/** One decision: which request it answers (`answerWith` is copied from the request) and what was decided. */
export interface DecisionAnswer {
  org: string
  team: string
  answerWith: AnswerWith
  /** The request's title. The request leaves the team's view once answered, so the node keeps the title in its log. */
  title: string
  choice?: string
  /** What the chosen option says, kept in the decision log because the request is gone from the team's view once answered. */
  choiceLabel?: string
  text: string
}
