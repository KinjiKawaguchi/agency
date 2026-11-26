import AgencyAPI
import Foundation

/// What each participant of a team did over the period, as the screen shows it (agency#364): how their time went, what they cost,
/// and the pull requests merged under their name. There is a section only when the node gave the participants: a team read by
/// someone other than its owner, or with a runtime that does not report them, has none, and nothing is drawn for it.
public struct ParticipantsSection: Equatable, Sendable {
    public struct Row: Equatable, Sendable, Identifiable {
        public var id: String { name }
        public let name: String
        /// How the participant's time went: its length and its share of the three together, in the order busy, waiting, idle.
        public let times: [TimePart]
        public let cost: String
        public let merged: String
        public let costPerPullRequest: String
        public let tokens: String
    }

    public struct TimePart: Equatable, Sendable, Identifiable {
        public enum Kind: Equatable, Sendable { case busy, waiting, idle }

        public var id: Kind { kind }
        public let kind: Kind
        public let label: String
        public let duration: String
        /// 0 to 1: this part's share of the three together; all 0 when the participant has no time at all.
        public let share: Double
    }

    public let rows: [Row]
    /// The pull requests no participant is named the author of, said only when there are some.
    public let unattributed: String?
    public let note = Ja.Evaluation.Participants.waitingNote

    /// `nil` when the node gave no participants.
    public init?(numbers: TeamEvaluation) {
        guard let participants = numbers.participants else { return nil }
        rows = participants.map(Self.row)
        unattributed = numbers.unattributedMerged.flatMap { $0 > 0 ? Ja.Evaluation.Participants.unattributed($0) : nil }
    }

    private static func row(_ participant: ParticipantEvaluation) -> Row {
        let total = participant.busySeconds + participant.waitingSeconds + participant.idleSeconds
        func part(_ kind: TimePart.Kind, _ label: String, _ seconds: Double) -> TimePart {
            TimePart(kind: kind, label: label, duration: DurationFormat.text(seconds), share: total > 0 ? seconds / total : 0)
        }
        return Row(
            name: participant.name,
            times: [
                part(.busy, Ja.Evaluation.Participants.busy, participant.busySeconds),
                part(.waiting, Ja.Evaluation.Participants.waiting, participant.waitingSeconds),
                part(.idle, Ja.Evaluation.Participants.idle, participant.idleSeconds),
            ],
            cost: JaFormat.money(participant.costUsd),
            merged: Ja.Evaluation.Participants.merged(participant.merged),
            costPerPullRequest: participant.costPerMergedPrUsd.map { Ja.Evaluation.Participants.costPerPullRequest(JaFormat.money($0)) }
                ?? Ja.Evaluation.Participants.noPullRequest,
            tokens: Ja.Evaluation.Participants.tokens(
                output: JaFormat.compact(participant.tokens.output), cacheRead: JaFormat.compact(participant.tokens.cacheRead)
            )
        )
    }
}
