import AgencyAPI
import Foundation

/// What each participant of a team did over the period, as the screen shows it (agency#364): how their time went, what they cost,
/// and the pull requests merged under their name. There is a section only when the node gave participants and there is at least
/// one: a team read by someone other than its owner, or with a runtime that does not report them, has none, and nothing is drawn
/// for it, an empty list included (the web does the same).
public struct ParticipantsSection: Equatable, Sendable {
    public struct Row: Equatable, Sendable, Identifiable {
        public var id: String { name }
        public let name: String
        /// How the participant's time went, in the order working, waiting, idle; none when no session of theirs was alive.
        public let times: [TimePart]
        /// What a screen reader says of the three times.
        public let timesSpoken: String
        public let cost: String
        /// All four kinds of token added up: the number says what it counts.
        public let tokens: String
        public let merged: String
        public let mergedSpoken: String
        /// Their cost over their merged pull requests, or "マージなし": never a $0 for a participant who merged nothing.
        public let costPerPullRequest: String
        public var hasSession: Bool { !times.isEmpty }
    }

    public struct TimePart: Equatable, Sendable, Identifiable {
        public enum Kind: Equatable, Sendable { case working, waiting, idle }

        public var id: Kind { kind }
        public let kind: Kind
        public let label: String
        public let duration: String
        /// 0 to 1: this part's share of the three together.
        public let share: Double
    }

    public let rows: [Row]
    /// The pull requests no participant is named the author of, said only when there are some.
    public let unattributed: String?

    /// `nil` when the node gave no participants, or an empty list.
    public init?(numbers: TeamEvaluation) {
        guard let participants = numbers.participants, !participants.isEmpty else { return nil }
        rows = participants.map(Self.row)
        unattributed = numbers.unattributedMerged.flatMap { $0 > 0 ? Ja.Evaluation.Participants.unattributed($0) : nil }
    }

    private static func row(_ participant: ParticipantEvaluation) -> Row {
        let total = participant.busySeconds + participant.waitingSeconds + participant.idleSeconds
        func part(_ kind: TimePart.Kind, _ label: String, _ seconds: Double) -> TimePart {
            TimePart(kind: kind, label: label, duration: DurationFormat.text(seconds), share: seconds / total)
        }
        let times = total > 0
            ? [
                part(.working, Ja.Evaluation.Participants.working, participant.busySeconds),
                part(.waiting, Ja.Evaluation.Participants.waiting, participant.waitingSeconds),
                part(.idle, Ja.Evaluation.Participants.idle, participant.idleSeconds),
            ]
            : []
        let tokens = participant.tokens.input + participant.tokens.output + participant.tokens.cacheRead + participant.tokens.cacheWrite
        return Row(
            name: participant.name,
            times: times,
            timesSpoken: Ja.Evaluation.Participants.timeSpoken(
                working: DurationFormat.text(participant.busySeconds), waiting: DurationFormat.text(participant.waitingSeconds),
                idle: DurationFormat.text(participant.idleSeconds)
            ),
            cost: JaFormat.money(participant.costUsd),
            tokens: Ja.Evaluation.Participants.tokens(JaFormat.integer(tokens)),
            merged: JaFormat.integer(participant.merged),
            mergedSpoken: Ja.Evaluation.Participants.mergedCount(participant.merged),
            costPerPullRequest: participant.costPerMergedPrUsd.map { JaFormat.money($0) } ?? Ja.Evaluation.Participants.noMerged
        )
    }
}
