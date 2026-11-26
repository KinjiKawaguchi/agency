import AgencyAPI
import Foundation

/// What a team needs from the viewer, most pressing first: waiting on them, stuck, nobody working, or fine.
public enum Attention: Int, Comparable, Sendable {
    case mine, stuck, idle, active

    public static func < (left: Attention, right: Attention) -> Bool { left.rawValue < right.rawValue }
}

/// How many of a team's work items are blocked for one reason.
public struct BlockedGroup: Equatable, Sendable {
    public let reason: BlockedReason
    public let count: Int
}

/// One team at a glance, worked out from what the team's runtime reports and what the node says is the viewer's.
public struct TeamSummary: Equatable, Sendable {
    public let attention: Attention
    /// Requests that are the viewer's to decide: the node's list, never worked out here.
    public let mine: Int
    /// The team's participants without the viewer, and how many of them are working.
    public let participants: Int
    public let working: Int
    /// Blocked work items by the reason the team's runtime gives, in a fixed order, reasons with none left out.
    public let blocked: [BlockedGroup]
    public let blockedCount: Int
    /// What the participants who are not working say they are doing, as the team reports it: once each, two lines at most.
    public let quiet: [String]

    private static let reasons: [BlockedReason] = [.needsDecision, .external, .waitingOnPrevious, .timeOrEquipment]
    private static let quietLines = 2

    public init(team: Team) {
        let participants = team.view.people.filter { $0.id != team.viewer }
        let working = participants.filter { $0.state == .working }.count
        let blocked = Self.reasons
            .map { reason in BlockedGroup(reason: reason, count: team.view.work.filter { $0.blocked?.reason == reason }.count) }
            .filter { $0.count > 0 }
        let blockedCount = blocked.reduce(0) { $0 + $1.count }
        var seen = Set<String>()
        let quiet = participants
            .filter { $0.state != .working }
            .map { $0.status.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty && seen.insert($0).inserted }
            .prefix(Self.quietLines)
        self.mine = team.mine.count
        self.participants = participants.count
        self.working = working
        self.blocked = blocked
        self.blockedCount = blockedCount
        self.quiet = Array(quiet)
        attention = if !team.mine.isEmpty {
            .mine
        } else if blockedCount > 0 {
            .stuck
        } else if !participants.isEmpty && working == 0 {
            .idle
        } else {
            .active
        }
    }
}
