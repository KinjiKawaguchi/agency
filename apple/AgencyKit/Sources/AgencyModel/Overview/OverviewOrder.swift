import AgencyAPI
import Foundation

/// What is known of one team: still loading, could not be read, or its snapshot.
public enum TeamState: Equatable, Sendable {
    case loading
    /// `reason` is the node's or the transport's words; `signInEnded` is true when the Access session is over.
    case failed(reason: String, signInEnded: Bool)
    case ready(Team)
}

/// One team of one org, as the overview lists it.
public struct TeamEntry: Equatable, Sendable {
    public let org: String
    public let ref: TeamRef
    public let state: TeamState

    public init(org: String, ref: TeamRef, state: TeamState) {
        self.org = org
        self.ref = ref
        self.state = state
    }

    public var key: String { "\(org)/\(ref.id)" }

    /// `nil` until the team has been read.
    public var summary: TeamSummary? {
        if case .ready(let team) = state { TeamSummary(team: team) } else { nil }
    }
}

/// The order the overview shows teams and orgs in: what needs the viewer first.
public enum OverviewOrder {
    /// A team that cannot be read comes after the stuck ones and before the idle ones; one still loading comes last.
    private static let failedRank = 2
    private static let loadingRank = 5

    public static func rank(_ state: TeamState) -> Int {
        switch state {
        case .failed: failedRank
        case .loading: loadingRank
        case .ready(let team):
            switch TeamSummary(team: team).attention {
            case .mine: 0
            case .stuck: 1
            case .idle: 3
            case .active: 4
            }
        }
    }

    /// Within one rank, the team with more waiting on the viewer or more blocked comes first, then by name.
    private static func weight(_ state: TeamState) -> Int {
        guard case .ready(let team) = state else { return 0 }
        let summary = TeamSummary(team: team)
        return summary.attention == .mine ? summary.mine : summary.blockedCount
    }

    public static func teams(_ entries: [TeamEntry]) -> [TeamEntry] {
        entries.sorted { left, right in
            let (leftRank, rightRank) = (rank(left.state), rank(right.state))
            if leftRank != rightRank { return leftRank < rightRank }
            let (leftWeight, rightWeight) = (weight(left.state), weight(right.state))
            if leftWeight != rightWeight { return leftWeight > rightWeight }
            return byName(left.ref.label, right.ref.label) ?? (left.key < right.key)
        }
    }

    /// An org comes where its most pressing team does. An org with no teams comes after every team that has been read.
    public static func rank(ofOrgWith entries: [TeamEntry]) -> Int {
        entries.map { rank($0.state) }.min().map { min($0, loadingRank) } ?? loadingRank
    }

    /// `nil` when the two names are the same.
    static func byName(_ left: String, _ right: String) -> Bool? {
        switch left.localizedStandardCompare(right) {
        case .orderedAscending: true
        case .orderedDescending: false
        case .orderedSame: nil
        }
    }
}
